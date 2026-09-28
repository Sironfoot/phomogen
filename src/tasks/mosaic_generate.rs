use std::{collections::HashMap, fs, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver}}, thread::{self, JoinHandle}};

use crate::{app::App, ffmpeg::frame_extractor::{FrameExtractor, ImageTileData, VideoFrameMatch}};

use anyhow::Result;
use image::{DynamicImage, ImageBuffer, Rgb, imageops};
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

        for (num_tiles_x, num_tiles_y, _, _, matched_tiles) in chosen_tiling_options {
            // create the temporary mosaic directory
            let temp_mosaic_dir_name = format!("{image_filename}_temp_{num_tiles_x}x{num_tiles_y}");
            let temp_mosaic_dir = database_dir.join(&temp_mosaic_dir_name);

            let temp_mosaic_exists = fs::exists(&temp_mosaic_dir).unwrap_or(false);
            if !temp_mosaic_exists {
                fs::create_dir(&temp_mosaic_dir).unwrap();
            }

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

                for _ in 0..num_workers {
                    let video_metadata = video.clone();
                    let frame_indices = Arc::clone(&frame_indices);
                    let next_index = Arc::clone(&next_index);
                    let tx = tx.clone();
                    
                    workers.push(thread::spawn(move || {
                        let mut frame_extractor = FrameExtractor::new(video_metadata);

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
        }
    });

    Ok(progress_receiver)
}