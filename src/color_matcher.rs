use std::{collections::HashMap, sync::mpsc::{self, Receiver}, thread::{self}};

use anyhow::Result;

use crate::app::{ImageFile, ImageTile, frame_data::{Color, VideoColorIndexDatabase}};

pub struct ColorMatcher<'database> {
    pub red_bias: u64,
    pub green_bias: u64,
    pub blue_bias: u64,

    pub mosaic_tiles_x: u32,
    pub mosaic_tiles_y: u32,

    num_workers: u32,

    databases: HashMap<String, &'database VideoColorIndexDatabase>,
}

impl<'database> ColorMatcher<'database> {
    pub fn new(mosaic_tiles_x: u32, mosaic_tiles_y: u32) -> Self {
        Self {
            red_bias: 3,
            green_bias: 6,
            blue_bias: 1,
            mosaic_tiles_x,
            mosaic_tiles_y,
            num_workers: 1,
            databases: HashMap::new(),
        }
    }

    pub fn thread_count(&self) -> u32 {
        self.num_workers
    }

    pub fn set_thread_count(&mut self, threads: u32) {
        self.num_workers = threads.max(1);
    }

    pub fn add_database(&mut self, video_file_name: &str, database: &'database VideoColorIndexDatabase) {
        self.databases.insert(String::from(video_file_name), database);
    }

    pub fn match_tiles(&self, image: &ImageFile) -> Result<Receiver<FrameMatch>> {
        let Some(image_tiles) = &image.image_tiles else {
            return Err(anyhow::format_err!("image doesn't contain any tile data"));
        };

        let num_mosaic_tiles = self.mosaic_tiles_x * self.mosaic_tiles_y;

        //let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(self.num_workers as usize);
        let (tx, rc) = mpsc::channel::<FrameMatch>();

        // 10 frames / 3 threads: 10 / 3 floored = 3
        let tiles_per_worker = f64::floor(num_mosaic_tiles as f64 / self.num_workers as f64) as u32;
        // remainder on division 10 / 3 = 1
        let remainder_tiles =  num_mosaic_tiles % self.num_workers;

        thread::scope(|scope| {
            for worker_index in 0..self.num_workers {
                let is_last = worker_index == (self.num_workers - 1);

                let starting_tile_index = worker_index * tiles_per_worker;

                //  10 tiles / 3 threads, thread 1 = 1,2,3, thread 2 = 4,5,6, thread 3 = 6,7,8,10
                let ending_tile_index = match is_last {
                    true => (starting_tile_index + tiles_per_worker) + remainder_tiles,
                    false => starting_tile_index + tiles_per_worker
                };

                let worker_tiles = &image_tiles[starting_tile_index as usize..ending_tile_index as usize];

                let database = &self.databases;
                let tx = tx.clone();

                scope.spawn(move || {
                    for (offset, tile) in worker_tiles.iter().enumerate() {
                        let tile_index = starting_tile_index + offset as u32;

                        let frame_match = self.find_nearest_color(database, tile, tile_index);
                        tx.send(frame_match).unwrap();
                    }
                });
            }

            drop(tx);
        });

        Ok(rc)
    }

    fn find_nearest_color(&self, databases: &HashMap<String, &VideoColorIndexDatabase>, candidate: &ImageTile, tile_index: u32) -> FrameMatch {
        let mut nearest_match = FrameMatch {
            tile_index,
            video_filename: String::new(),
            frame_index: 0,
            crop_resize: 100.0,
            crop_pos_x: 0.0,
            crop_pos_y: 0.0,
            is_flipped: false,
        };

        let mut smallest_distance = std::u64::MAX;
        
        for (video_filename, database) in databases {
            let mut match_found_in_database = false;

            for frame in database.frames() {
                for crop in &frame.crops {
                    // check non-flipped
                    let distance = self.check_distance(&crop.colors, candidate);

                    if distance < smallest_distance {
                        nearest_match.tile_index = tile_index;
                        nearest_match.frame_index =  frame.frame_index;
                        nearest_match.crop_resize = crop.resize_percentage;
                        nearest_match.crop_pos_x = crop.pos_x_percentage;
                        nearest_match.crop_pos_y = crop.pos_y_percentage;
                        nearest_match.is_flipped = false;

                        smallest_distance = distance;
                        match_found_in_database = true;
                    }
                    
                    // check flipped
                    let flipped_distance = self.check_distance(crop.colors_flipped(), candidate);

                    if flipped_distance < smallest_distance {
                        nearest_match.tile_index = tile_index;
                        nearest_match.frame_index =  frame.frame_index;
                        nearest_match.crop_resize = crop.resize_percentage;
                        nearest_match.crop_pos_x = crop.pos_x_percentage;
                        nearest_match.crop_pos_y = crop.pos_y_percentage;
                        nearest_match.is_flipped = true;

                        smallest_distance = flipped_distance;
                        match_found_in_database = true;
                    }
                }
            }

            if match_found_in_database {
                nearest_match.video_filename = video_filename.clone();
            }
        }

        nearest_match
    }

    fn check_distance<'color, I>(&self, frame_colors: I, candidate: &ImageTile) -> u64
    where
        I: IntoIterator<Item = &'color Color>,
    {
        let mut frame_colors = frame_colors.into_iter();
        let mut distance: u64 = 0;

        for image_color in &candidate.colors {
            let Some(frame_color) = frame_colors.next() else {
                return u64::MAX;
            };

            let red_dist = frame_color.r.abs_diff(image_color.r) as u64;
            let green_dist = frame_color.g.abs_diff(image_color.g) as u64;
            let blue_dist = frame_color.b.abs_diff(image_color.b) as u64;

            distance +=
                (self.red_bias * red_dist * red_dist) +
                (self.green_bias * green_dist * green_dist) +
                (self.blue_bias * blue_dist * blue_dist);
        }

        distance
    }
}

#[derive(Debug)]
pub struct FrameMatch {
    pub tile_index: u32,

    pub video_filename: String,
    pub frame_index: u32,
    
    pub crop_resize: f64,
    pub crop_pos_x: f64,
    pub crop_pos_y: f64,
    pub is_flipped: bool,
}