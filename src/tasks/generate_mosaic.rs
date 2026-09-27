use std::{collections::HashMap, fs, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver}}, thread::{self, JoinHandle}};

use crate::{app::App, ffmpeg::frame_extractor::{FrameExtractor, ImageTileData, VideoFrameMatch}, tile_blender::TileBlender};

use anyhow::Result;
use image::{DynamicImage, GenericImage, ImageBuffer, ImageEncoder, Rgb, RgbImage, codecs::{jpeg::JpegEncoder, png::PngEncoder}, imageops};
use image::imageops::FilterType;

const LARGEST_PRINT_DIMENSION: u32 = 14000;
const LARGEST_SOCIAL_DIMENSION: u32 = 7680;

pub struct Response {
    pub num_tiles_x: u8,
    pub num_tiles_y: u8,
    pub tile_index: u32,
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
    let max_allowed_cores = app.system_info.max_allowed_cores();

    let mut video_filenames = chosen_tiling_options.iter()
        .flat_map(|(_, _, _, _, matched_tiles)| matched_tiles.iter().map(|t| t.video_filename.clone()))
        .collect::<Vec<_>>();

    video_filenames.dedup();

    let videos = app.videos.iter()
        .filter(|v| video_filenames.contains(&v.metadata.file_name))
        .map(|v| v.metadata.clone())
        .collect::<Vec<_>>();

    let mosaics_dir = app.mosaics_dir.clone();
    let database_dir = app.database_dir.clone();
    let srgb_profile = app.get_srgb_profile();
    let (temp_tile_width, temp_tile_height) = select_tile_shape.get_temp_image_tile_dimensions();

    thread::spawn(move || {
        // create the database folder
        let database_dir_exists = fs::exists(&database_dir).unwrap_or(false);
        if !database_dir_exists {
            fs::create_dir(&database_dir).unwrap();
        }

        // create the temporary mosaic directory
        let temp_mosaic_dir_name = format!("{image_filename}_temp");
        let temp_mosaic_dir = database_dir.join(&temp_mosaic_dir_name);

        let temp_mosaic_exists = fs::exists(&temp_mosaic_dir).unwrap_or(false);
        if !temp_mosaic_exists {
            fs::create_dir(&temp_mosaic_dir).unwrap();
        }

        for (num_tiles_x, num_tiles_y, width, height, matched_tiles) in chosen_tiling_options {
            let mut video_frame_matches: HashMap<&str, Vec<VideoFrameMatch>> = HashMap::new();

            // group into videos
            for frame_match in &matched_tiles {
                let entry = video_frame_matches
                    .entry(frame_match.video_filename.as_str())
                    .or_insert_with(|| Vec::new());

                entry.push(VideoFrameMatch {
                    tile_index: frame_match.tile_index,
                    frame_index: frame_match.frame_index,
                    crop_resize: frame_match.crop_resize,
                    crop_pos_x: frame_match.crop_pos_x,
                    crop_pos_y: frame_match.crop_pos_y,
                    is_flipped: frame_match.is_flipped,
                });
            }

            let num_workers = max_allowed_cores / 1;

            for (video_filname, video_frame_matches) in video_frame_matches {
                let Some(video) = videos.iter()
                    .find(|v| v.file_name == video_filname) else { continue; };

                let mut frame_indices = video_frame_matches.iter()
                    .map(|frame| frame.frame_index)
                    .collect::<Vec<u32>>();

                frame_indices.sort_unstable();
                frame_indices.dedup();

                let total_frame_indices = frame_indices.len();

                // ensure total threads aren't more than total matches
                let num_workers = (num_workers as usize).min(total_frame_indices);
            
                let next_index = Arc::new(AtomicUsize::new(0));
                let frame_indices = Arc::new(frame_indices);
                let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers as usize);

                let (tx, rc) = mpsc::channel::<(u32, ImageBuffer<Rgb<u8>, Vec<u8>>)>();

                for worker_index in 0..num_workers {
                    let video_metadata = video.clone();
                    let frame_indices = Arc::clone(&frame_indices);
                    let next_index = Arc::clone(&next_index);
                    let tx = tx.clone();
                    
                    workers.push(thread::spawn(move || {
                        let mut frame_extractor = FrameExtractor::new(worker_index as u32, video_metadata);

                        loop {
                            let index = next_index.fetch_add(1, Ordering::Relaxed);

                            // stop the thread when no more tiles left
                            if index >= total_frame_indices {
                                break;
                            }

                            let frame_index = frame_indices[index];
                            let image = frame_extractor.extract(frame_index).unwrap();
                        
                            if tx.send((frame_index, image)).is_err() {
                                // The receiver was dropped, so stop working.
                                break;
                            }
                        }
                    }));
                }

                drop(tx);

                for (frame_index, image) in rc {
                    let matches = video_frame_matches.iter()
                        .filter(|frame_match| frame_match.frame_index == frame_index);

                    let (frame_width, frame_height) = image.dimensions();

                    for matched_frame in matches {
                        let pos_x = f64::round((frame_width as f64 / 100.0) * matched_frame.crop_pos_x as f64) as u32;
                        let pos_y = f64::round((frame_height as f64 / 100.0) * matched_frame.crop_pos_y as f64) as u32;
                    
                        let cropped_width = f64::round((frame_width as f64 / 100.0) * matched_frame.crop_resize as f64) as u32;
                        let cropped_height = f64::round((frame_height as f64 / 100.0) * matched_frame.crop_resize as f64) as u32;
                    
                        let mut crop = imageops::crop_imm(&image, pos_x, pos_y, cropped_width, cropped_height).to_image();
                        
                        if matched_frame.is_flipped {
                            imageops::flip_horizontal_in_place(&mut crop);
                        }

                        let tile = ImageTileData {
                            tile_index: matched_frame.tile_index,
                            data: DynamicImage::ImageRgb8(crop),
                        };

                        let row = tile.tile_index / num_tiles_x as u32;
                        let col = tile.tile_index % num_tiles_x as u32;

                        let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));

                        let resized = imageops::resize(&tile.data, temp_tile_width, temp_tile_height, FilterType::Lanczos3);
                        resized.save(tile_path).unwrap();

                        progress_sender.send(Response {
                            num_tiles_x: num_tiles_x,
                            num_tiles_y: num_tiles_y,
                            tile_index: tile.tile_index,
                        }).unwrap();
                    }
                }

                for worker in workers {
                    worker.join().unwrap();
                }
            }

            // join image
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
        }
    });

    Ok(progress_receiver)
}