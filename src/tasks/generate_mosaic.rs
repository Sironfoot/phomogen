use std::{collections::HashMap, fs, sync::mpsc::{self, Receiver}, thread::{self, JoinHandle}};

use crate::{app::App, ffmpeg::frame_extractor::{FrameExtractor, ImageTileData, VideoFrameMatch}, tile_blender::TileBlender};

use anyhow::Result;
use image::{GenericImage, ImageBuffer, ImageEncoder, Rgb, RgbImage, codecs::{jpeg::JpegEncoder, png::PngEncoder}, imageops};
use image::imageops::FilterType;
pub struct MosaicGenerationReport {
    pub tile_index: u32,
    pub row: u32,
    pub col: u32,
}

pub fn run(app: &App) -> Result<Receiver<MosaicGenerationReport>> {
    let (progress_sender, progress_receiver) = mpsc::channel::<MosaicGenerationReport>();

    let image_result = app.images.iter()
        .find(|i| i.is_chosen);

    let Some(chosen_image) = image_result else {
        return Err(anyhow::format_err!("No chosen image found"));
    };

    let Some(frame_matches) = &chosen_image.matched_tiles else {
        return Err(anyhow::format_err!("Image has no matched tiles"));
    };

    // copy what we need
    let image_filename = chosen_image.file_name.clone();

    let mosaic_tiles_x = app.mosaic_tiles_x;
    let mosaic_tiles_y = app.mosaic_tiles_y;

    let max_allowed_cores = app.system_info.max_allowed_cores();

    let frame_matches = frame_matches.clone();
    let mut video_filenames = frame_matches.iter()
        .map(|f| f.video_filename.clone())
        .collect::<Vec<_>>();

    video_filenames.dedup();

    let videos = app.videos.iter()
        .filter(|v| video_filenames.contains(&v.metadata.file_name))
        .map(|v| v.metadata.clone())
        .collect::<Vec<_>>();

    let mosaics_dir = app.mosaics_dir.clone();
    let database_dir = app.database_dir.clone();
    let srgb_profile = app.get_srgb_profile();

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

        let mut video_frame_matches: HashMap<&str, Vec<VideoFrameMatch>> = HashMap::new();

        // group into videos
        for frame_match in &frame_matches {
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
            if let Some(video) = videos.iter().find(|v| v.file_name == video_filname) {
                let total_matches = video_frame_matches.len() as u32;
                // ensure total threads aren't more than total matches
                let num_workers = num_workers.min(total_matches);
            
                let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers as usize);
                let (tx, rc) = mpsc::channel::<ImageTileData>();

                // 10 matches / 3 threads: 10 / 3 floored = 3
                let matches_per_worker = f64::floor(total_matches as f64 / num_workers as f64) as u32;
                // remainder on division 10 / 3 = 1
                let remainder_matches = total_matches as u32 % num_workers as u32;

                for worker_index in 0..num_workers {
                    let is_last = worker_index == (num_workers - 1);

                    let starting_match_index = worker_index as u32 * matches_per_worker;

                    //  10 matches / 3 threads, thread 1 = 1,2,3, thread 2 = 4,5,6, thread 3 = 6,7,8,10
                    let ending_match_index = match is_last {
                        true => (starting_match_index + matches_per_worker) + remainder_matches,
                        false => starting_match_index + matches_per_worker
                    };

                    let workers_matches = video_frame_matches[
                        starting_match_index as usize..ending_match_index as usize].to_vec().clone();

                    let video_metadata = video.clone();
                    let tx = tx.clone();
                    
                    workers.push(thread::spawn(move || {
                        let mut frame_extractor = FrameExtractor::new(worker_index, video_metadata);
                        frame_extractor.run(&workers_matches, tx).unwrap();
                    }));
                }

                drop(tx);

                for tile in rc {
                    let row = tile.tile_index / mosaic_tiles_x;
                    let col = tile.tile_index % mosaic_tiles_x;

                    let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));

                    // TODO: don't hard code dimensions
                    let resized = imageops::resize(&tile.data, 960, 540, FilterType::Triangle);
                    resized.save(tile_path).unwrap();

                    progress_sender.send(MosaicGenerationReport {
                        tile_index: tile.tile_index,
                        row,
                        col,
                    }).unwrap();
                }

                for worker in workers {
                    worker.join().unwrap();
                }
            }
        }

        // join images
        let image_width: u32 = 7680;
        let image_height: u32 = 4320;

        let ratio = image_width as f64 / image_height as f64;

        const LARGEST_DIMENSION: u32 = 10000;
        let smallest_dimension = (LARGEST_DIMENSION as f64 / ratio).round() as u32;

        let is_landscape = image_width > image_height;

        let target_width = if is_landscape { LARGEST_DIMENSION } else { smallest_dimension };

        let tile_width = (target_width as f64 / mosaic_tiles_x as f64).round() as u32;
        let tile_height = (tile_width as f64 / ratio as f64).round() as u32;

        let final_width = tile_width * mosaic_tiles_x;
        let final_height = tile_height * mosaic_tiles_y;
        
        let mut canvas: ImageBuffer<Rgb<u8>, Vec<u8>> = RgbImage::new(final_width, final_height);

        for frame_match in frame_matches {
            let row = frame_match.tile_index / mosaic_tiles_x;
            let col = frame_match.tile_index % mosaic_tiles_x;

            let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
            let tile_image = image::open(tile_path).unwrap();
            let mut image = tile_image.into_rgb8();

            let tile_blender = TileBlender::new(row, col, mosaic_tiles_x, mosaic_tiles_y);

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
                &image, tile_width, tile_height, FilterType::Triangle);

            canvas.copy_from(&resized, col * tile_width, row * tile_height).unwrap();
        }

        // create print quality version
        let mosaic_image_name = format!("{image_filename}_{mosaic_tiles_x}x{mosaic_tiles_y}.png");
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

        // create smaller version
        let mosaic_image_name = format!("{image_filename}_{mosaic_tiles_x}x{mosaic_tiles_y}.jpeg");
        let image_path = mosaics_dir.join(&mosaic_image_name);

        let file_write = fs::File::create_new(&image_path).unwrap();
        let mut encoder = JpegEncoder::new_with_quality(file_write, 99);
        encoder.set_icc_profile(srgb_profile).unwrap();

        let jpeg_width: u32 = 7680;
        let jpeg_height: u32 = 4320;

        let jpeg_canvas = imageops::resize(&canvas, jpeg_width, jpeg_height, FilterType::Lanczos3);
        encoder.write_image(
            jpeg_canvas.as_raw(),
            jpeg_canvas.width(),
            jpeg_canvas.height(),
            image::ExtendedColorType::Rgb8
        ).unwrap();

        fs::remove_dir_all(&temp_mosaic_dir).unwrap();
    });

    Ok(progress_receiver)
}