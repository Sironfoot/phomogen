use std::{fs, sync::mpsc::{self, Receiver}, thread};

use anyhow::Result;
use image::{GenericImage, ImageBuffer, ImageEncoder, Rgb, RgbImage, codecs::{jpeg::JpegEncoder, png::PngEncoder}, imageops};
use image::imageops::FilterType;

use crate::{app::App, tile_blender::TileBlender};

const LARGEST_PRINT_DIMENSION: u32 = 14000;
const LARGEST_SOCIAL_DIMENSION: u32 = 7680;

pub struct Response {
    pub num_tiles_x: u8,
    pub num_tiles_y: u8,
    pub tile_index: Option<u32>,
    pub is_finished: bool,
}

pub fn run(app: &App) -> Result<Receiver<Response>> {
    let (progress_sender, progress_receiver) = mpsc::channel::<Response>();

    let Some(chosen_image) = app.images.iter().find(|i| i.is_chosen) else {
        return Err(anyhow::format_err!("no image selected"));
    };

    let select_tile_shape = &app.selected_tile_shape;
    let Some(tiling_options) = chosen_image.tiling_options.get(select_tile_shape) else {
        return Err(anyhow::format_err!("mosaic tiling layouts not found for selected tile shape"));
    };

    let chosen_tiling_options = tiling_options.iter()
        .filter(|t|
            t.is_chosen &&
            t.matched_tiles.is_some())
        .filter_map(|t| {
            let num_tiles_x = t.num_tiles_x;
            let num_tiles_y = t.num_tiles_y;
            let width = t.cropped_width;
            let height = t.cropped_height;
            let Some(matched_tiles) = &t.matched_tiles else { return None; };

            Some((num_tiles_x, num_tiles_y, width, height, matched_tiles.clone()))
        })
        .collect::<Vec<_>>();

    if chosen_tiling_options.len() == 0 {
        return Err(anyhow::format_err!("No tile layouts have been chosen"));
    }

    // clone what we need
    let image_filename = chosen_image.file_name.clone();
    //let max_allowed_cores = app.system_info.max_allowed_cores();

    let mosaics_dir = app.mosaics_dir.clone();
    let database_dir = app.database_dir.clone();
    let srgb_profile = app.get_srgb_profile();

    thread::spawn(move || {
        for (num_tiles_x, num_tiles_y, width, height, matched_tiles) in chosen_tiling_options {
            // create the temporary mosaic directory
            let temp_mosaic_dir_name = format!("{image_filename}_temp_{num_tiles_x}x{num_tiles_y}");
            let temp_mosaic_dir = database_dir.join(&temp_mosaic_dir_name);

            let is_landscape = width > height;
            let ratio = width as f64 / height as f64;

            let smallest_dimension = if is_landscape {
                (LARGEST_PRINT_DIMENSION as f64 / ratio).round() as u32
            }
            else
            {
                (LARGEST_PRINT_DIMENSION as f64 * ratio).round() as u32
            };

            let target_width: u32 = if is_landscape { LARGEST_PRINT_DIMENSION } else { smallest_dimension };
            let target_height: u32 = if is_landscape { smallest_dimension } else { LARGEST_PRINT_DIMENSION };

            let tile_width = (target_width as f64 / num_tiles_x as f64).round() as u32;
            let tile_height = (target_height as f64 / num_tiles_y as f64).round() as u32;

            let final_width = tile_width * num_tiles_x as u32;
            let final_height = tile_height * num_tiles_y as u32;

            let mut canvas: ImageBuffer<Rgb<u8>, Vec<u8>> = RgbImage::new(final_width, final_height);

            for frame_match in &matched_tiles {
                let row = frame_match.tile_index / num_tiles_x as u32;
                let col = frame_match.tile_index % num_tiles_x as u32;

                let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
                let tile_image = image::open(tile_path).unwrap();
                let mut image = tile_image.into_rgb8();

                let tile_blender = TileBlender::new(row, col, num_tiles_x as u32, num_tiles_y as u32);

                let top_image = tile_blender.find_top().map(|(row, col)| {
                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
                    image::open(tile_path).unwrap().into_rgb8()
                });

                let right_image = tile_blender.find_right().map(|(row, col)| {
                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
                    image::open(tile_path).unwrap().into_rgb8()
                });

                let bottom_image = tile_blender.find_bottom().map(|(row, col)| {
                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
                    image::open(tile_path).unwrap().into_rgb8()
                });

                let left_image = tile_blender.find_left().map(|(row, col)| {
                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
                    image::open(tile_path).unwrap().into_rgb8()
                });

                tile_blender.blend_image(&mut image, top_image.as_ref(), right_image.as_ref(), bottom_image.as_ref(), left_image.as_ref()).unwrap();

                let resized = imageops::resize(
                    &image, tile_width, tile_height, FilterType::Lanczos3);

                canvas.copy_from(&resized, col * tile_width, row * tile_height).unwrap();

                progress_sender.send(Response {
                    num_tiles_x: num_tiles_x,
                    num_tiles_y: num_tiles_y,
                    tile_index: Some(frame_match.tile_index),
                    is_finished: false,
                }).unwrap();
            }

            // create print quality version
            let mosaic_image_name = format!("{image_filename}_{num_tiles_x}x{num_tiles_y}.png");
            let image_path = mosaics_dir.join(&mosaic_image_name);

            let file_write = fs::File::create_new(&image_path).unwrap();

            let mut encoder = PngEncoder::new(file_write);
            encoder.set_icc_profile(srgb_profile.clone()).unwrap();
            encoder.write_image(
                canvas.as_raw(),
                canvas.width(),
                canvas.height(),
                image::ExtendedColorType::Rgb8
            ).unwrap();

            // create smaller version for social media etc.
            let mosaic_image_name = format!("{image_filename}_{num_tiles_x}x{num_tiles_y}.jpeg");
            let image_path = mosaics_dir.join(&mosaic_image_name);

            let file_write = fs::File::create_new(&image_path).unwrap();
            let mut encoder = JpegEncoder::new_with_quality(file_write, 99);
            encoder.set_icc_profile(srgb_profile.clone()).unwrap();

            let smallest_social_dimension = if is_landscape {
                (LARGEST_SOCIAL_DIMENSION as f64 / ratio).round() as u32
            }
            else
            {
                (LARGEST_SOCIAL_DIMENSION as f64 * ratio).round() as u32
            };

            let jpeg_width: u32 = if is_landscape { LARGEST_SOCIAL_DIMENSION } else { smallest_social_dimension }; // 14000
            let jpeg_height: u32 = if is_landscape { smallest_social_dimension } else { LARGEST_SOCIAL_DIMENSION }; // 7876

            let jpeg_canvas = imageops::resize(&canvas, jpeg_width, jpeg_height, FilterType::Lanczos3);
            encoder.write_image(
                jpeg_canvas.as_raw(),
                jpeg_canvas.width(),
                jpeg_canvas.height(),
                image::ExtendedColorType::Rgb8
            ).unwrap();

            fs::remove_dir_all(&temp_mosaic_dir).unwrap();

            progress_sender.send(Response {
                num_tiles_x: num_tiles_x,
                num_tiles_y: num_tiles_y,
                tile_index: None,
                is_finished: true,
            }).unwrap();
        }
    });

    Ok(progress_receiver)
}