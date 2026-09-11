use std::{collections::HashMap, fs::File, io::{self, BufRead}, sync::mpsc::{self, Receiver}, thread};

use crate::{app::{App, frame_data::{Color, FrameCrop, FrameData, VideoColorIndexDatabase}}, ffmpeg::crops::CropLevel};

pub struct LoadDatabaseProgressReport {
    pub video_file_name: String,
    pub total_frames_processed: u32,
    pub dropped_frames: u32,
    pub database: Option<VideoColorIndexDatabase>,
}

pub fn run(app: &App) -> Receiver<LoadDatabaseProgressReport> {
    let (tx, rc) = mpsc::channel::<LoadDatabaseProgressReport>();

    const REPORT_PROGRESS_AFTER_FRAMES: u32 = 1234;

    let color_tiles_x = app.color_tiles_x;
    let color_tiles_y = app.color_tiles_y;
    let total_colors = (color_tiles_x * color_tiles_y) as usize;

    let videos_to_load = app.videos.iter()
        .filter(|v| v.is_chosen && v.database.is_none() && v.database_path.is_some())
        .collect::<Vec<_>>();

    for video in videos_to_load {
        let video_file_name = video.metadata.file_name.clone();
        let total_frames = video.metadata.total_frames as u32;
        let database_path = video.database_path.clone().unwrap();
        let tx = tx.clone();

        thread::spawn(move || {
            let mut total_frames_added: u32 = 0;

            let mut frames: HashMap<u32, FrameData> = HashMap::with_capacity(total_frames as usize);

            let data_file = File::open(&database_path).unwrap();
            let mut reader = io::BufReader::with_capacity(256 * 1024, data_file);

            let mut dropped_frames: u32 = 0;

            let max_line_length = 222;
            let mut line = String::with_capacity(max_line_length);
            
            // e.g. 0 100 0 0 1 108,105,100 99,96,99 85,85,77....
            loop {
                line.clear();

                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {},
                    Err(_) => break,
                }

                let mut parts = line.split_ascii_whitespace();

                // since the database is a basic text file, we needto deal with the potential
                // that it's been opened and tampered with

                // get frame index
                let Some(frame_index) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
                    dropped_frames += 1;
                    continue;
                };

                // check frame index is valid, can't be more than frames in the video
                if (frame_index + 1) > total_frames {
                    dropped_frames += 1;
                    continue;
                }

                // get resize precentage
                let Some(resize_percentage) = parts.next().and_then(|v| v.parse::<f32>().ok()) else {
                    dropped_frames += 1;
                    continue;
                };

                // should be in range 1 - 100
                if resize_percentage < 1.0 || resize_percentage > 100.0 {
                    dropped_frames += 1;
                    continue;
                }

                // get pos X
                let Some(pos_x_percentage) = parts.next().and_then(|v| v.parse::<f32>().ok()) else {
                    dropped_frames += 1;
                    continue;
                };

                if pos_x_percentage > 100.0 {
                    dropped_frames += 1;
                    continue;
                }

                // get pos Y
                let Some(pos_y_percentage) = parts.next().and_then(|v| v.parse::<f32>().ok()) else {
                    dropped_frames += 1;
                    continue;
                };

                if pos_y_percentage > 100.0 {
                    dropped_frames += 1;
                    continue;
                }

                // get crop level
                let Some(crop_level) = parts.next().and_then(|v| v.parse::<u8>().ok()) else {
                    dropped_frames += 1;
                    continue;
                };

                let Ok(crop_level) = CropLevel::try_from(crop_level) else {
                    dropped_frames += 1;
                    continue;
                };

                let mut colors: Vec<Color> = Vec::with_capacity(total_colors);

                while let Some(color) = parts.next() {
                    let mut rgb= color.split(',');

                    let Some(r) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                        continue;
                    };

                    let Some(g) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                        continue;
                    };

                    let Some(b) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                        continue;
                    };

                    colors.push(Color { r, g, b });
                }

                // number of colors should match number of tiles
                if colors.len() != total_colors {
                    dropped_frames += 1;
                    continue;
                }

                let mut crop = FrameCrop::init(
                    color_tiles_x as u8,
                    resize_percentage,
                    pos_x_percentage,
                    pos_y_percentage,
                    crop_level);

                crop.colors = colors;

                let mut new_frame_added = false;

                let frame_data = frames
                    .entry(frame_index)
                    .or_insert_with(|| {
                        new_frame_added = true;
                        FrameData::new(frame_index)
                    });

                if new_frame_added {
                    total_frames_added += 1;
                }

                frame_data.crops.push(crop);

                if new_frame_added && total_frames_added % REPORT_PROGRESS_AFTER_FRAMES == 0 {
                    tx.send(LoadDatabaseProgressReport {
                        video_file_name: video_file_name.clone(),
                        total_frames_processed: total_frames_added,
                        dropped_frames,
                        database: None,
                    }).unwrap();
                }
            }

            if dropped_frames > 0 {
                panic!("DROPPED FRAMES DETECTED!!");
            }

            let color_database = VideoColorIndexDatabase::new(
                color_tiles_x, color_tiles_y, frames.into_values().collect());

            tx.send(LoadDatabaseProgressReport {
                video_file_name: video_file_name,
                total_frames_processed: total_frames_added,
                dropped_frames,
                database: Some(color_database),
            }).unwrap();
        });
    }

    drop(tx);

    rc
}