use std::{collections::HashMap, fs, io::{BufWriter, Write}, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver}}, thread::{self, JoinHandle}};

use crate::{app::{App, MosaicTilingOption}, ffmpeg::frame_extractor::{FrameExtractor, ImageTileData, VideoFrameMatch}};

use anyhow::Result;
use image::{DynamicImage, ImageBuffer, ImageEncoder, Rgb, codecs::png::PngEncoder, imageops};
use image::imageops::FilterType;

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

    let database_dir = app.database_dir.clone();
    let (temp_tile_width, temp_tile_height) = select_tile_shape.get_temp_image_tile_dimensions();

    thread::spawn(move || {
        // create the database folder
        let database_dir_exists = fs::exists(&database_dir).unwrap_or(false);
        if !database_dir_exists {
            fs::create_dir(&database_dir).unwrap();
        }

        let videos = Arc::new(videos);

        for (num_tiles_x, num_tiles_y, _, _, matched_tiles) in chosen_tiling_options {
            // create the temporary mosaic directory
            let temp_mosaic_dir_name = format!("{image_filename}_temp_{num_tiles_x}x{num_tiles_y}");
            let temp_mosaic_dir = database_dir.join(&temp_mosaic_dir_name);

            let temp_mosaic_exists = fs::exists(&temp_mosaic_dir).unwrap_or(false);
            if !temp_mosaic_exists {
                fs::create_dir(&temp_mosaic_dir).unwrap();
            }

            let num_workers = max_allowed_cores * 1;

            // group by video filename and frame index
            let mut unique_video_frames: HashMap<(String, u32), Vec<VideoFrameMatch>> = HashMap::new();
            let mut keys: Vec<(String, u32)> = Vec::new();

            for matched_tile in &matched_tiles {
                let entry = unique_video_frames
                    .entry((matched_tile.video_filename.clone(), matched_tile.frame_index))
                    .or_insert_with(|| {
                        keys.push((matched_tile.video_filename.clone(), matched_tile.frame_index));

                        Vec::new()
                    });

                entry.push(VideoFrameMatch {
                    tile_index: matched_tile.tile_index,
                    frame_index: matched_tile.frame_index,
                    crop_resize: matched_tile.crop_resize,
                    crop_pos_x: matched_tile.crop_pos_x,
                    crop_pos_y: matched_tile.crop_pos_y,
                    is_flipped: matched_tile.is_flipped,
                });
            }

            // ensure total threads aren't more than total matches
            let num_workers = (num_workers as usize).min(keys.len());
        
            let next_index = Arc::new(AtomicUsize::new(0));
            let keys = Arc::new(keys);
            let videos = Arc::clone(&videos);
            let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers as usize);

            let (tx, rc) = mpsc::channel::<(String, u32, ImageBuffer<Rgb<u8>, Vec<u8>>)>();

            for _ in 0..num_workers {
                let next_index = Arc::clone(&next_index);
                let tx = tx.clone();
                let keys = Arc::clone(&keys);
                let videos = Arc::clone(&videos);
                
                workers.push(thread::spawn(move || {
                    loop {
                        let index = next_index.fetch_add(1, Ordering::Relaxed);
                    
                        // stop the thread when no more tiles left
                        if index >= keys.len() {
                            break;
                        }

                        let (video_filename, frame_index) = &keys[index];
                        
                        let Some(video) = videos.iter()
                            .find(|v| v.file_name == *video_filename) else { continue; };

                        let mut frame_extractor = FrameExtractor::new(video.clone());
                        let image = frame_extractor.extract(*frame_index).unwrap();
                    
                        if tx.send((video_filename.clone(), *frame_index, image)).is_err() {
                            // The receiver was dropped, so stop working.
                            break;
                        }
                    }
                }));
            }

            drop(tx);

            for (video_filename, frame_index, image) in rc {
                let Some(matches) = unique_video_frames
                    .get(&(video_filename, frame_index)) else { continue; };

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

                    let (row, col) = MosaicTilingOption::row_col_by_index(num_tiles_x, tile.tile_index);
                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));

                    let file_write = fs::File::create_new(&tile_path).unwrap();
                    let mut writer = BufWriter::with_capacity(256 * 1024, file_write);
                    let encoder = PngEncoder::new(&mut writer);

                    let resized = imageops::resize(
                        tile.data.as_rgb8().unwrap(),
                        temp_tile_width,
                        temp_tile_height,
                        FilterType::Lanczos3);

                    encoder.write_image(
                        resized.as_raw(),
                        resized.width(),
                        resized.height(),
                        image::ExtendedColorType::Rgb8
                    ).unwrap();

                    writer.flush().unwrap();

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
    });

    Ok(progress_receiver)
}