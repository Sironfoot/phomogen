use std::{collections::HashMap, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver}}, thread::{self}};

use anyhow::Result;

use crate::{app::frame_data::{Color, OklabColor, VideoColorIndexDatabase}, ffmpeg::crops::CropLevel};

#[derive(Clone, Debug)]
pub enum ColorMatcherAlgorithm {
    Rgb,
    RgbWeighted { red_bias: u8, green_bias: u8, blue_bias: u8 },
    Oklab,
    OklabWeighted { lightness_weight: f32, chroma_weight: f32 },
}

pub struct ColorMatcher {
    pub mosaic_tiles_x: u32,
    pub mosaic_tiles_y: u32,

    algorithm: ColorMatcherAlgorithm,

    num_workers: u32,
    allowed_crops: Vec<CropLevel>,

    databases: HashMap<String, Arc<VideoColorIndexDatabase>>,
}

impl ColorMatcher {
    pub fn new(mosaic_tiles_x: u32, mosaic_tiles_y: u32, allowed_crops: &[CropLevel]) -> Self {
        Self {
            mosaic_tiles_x,
            mosaic_tiles_y,
            algorithm: ColorMatcherAlgorithm::Oklab,
            num_workers: 1,
            allowed_crops: allowed_crops.to_vec(),
            databases: HashMap::new(),
        }
    }

    pub fn thread_count(&self) -> u32 {
        self.num_workers
    }

    pub fn set_thread_count(&mut self, threads: u32) {
        self.num_workers = threads.max(1);
    }

    pub fn add_database(&mut self, video_file_name: &str, database: &Arc<VideoColorIndexDatabase>) {
        self.databases.insert(String::from(video_file_name), Arc::clone(database));
    }

    pub fn algorithm(&self) -> &ColorMatcherAlgorithm {
        &self.algorithm
    }

    pub fn set_algorithm(&mut self, algorithm: ColorMatcherAlgorithm) {
        self.algorithm = algorithm;
    }

    pub fn match_tiles(self, image_tiles: &Arc<Vec<ImageTile>>) -> Result<Receiver<FrameMatch>> {
        let image_tiles = Arc::clone(image_tiles);
        let matcher = Arc::new(self);

        let num_mosaic_tiles = (matcher.mosaic_tiles_x * matcher.mosaic_tiles_y) as usize;
        let num_workers = (matcher.num_workers as usize).min(num_mosaic_tiles);
        
        let next_tile_index = Arc::new(AtomicUsize::new(0));
        let (tx, rc) = mpsc::channel::<FrameMatch>();

        for _ in 0..num_workers {
            let matcher = Arc::clone(&matcher);
            let image_tiles = Arc::clone(&image_tiles);
            let next_tile_index = Arc::clone(&next_tile_index);
            let algorithm = matcher.algorithm.clone();
            let tx = tx.clone();

            thread::spawn(move || {
                loop {
                    let tile_index = next_tile_index.fetch_add(1, Ordering::Relaxed);

                    // stop the thread when no more tiles left
                    if tile_index >= num_mosaic_tiles {
                        break;
                    }

                    let tile = &image_tiles[tile_index];
                    let frame_match = match algorithm {
                        ColorMatcherAlgorithm::Rgb => {
                            matcher.find_nearest_color(tile, tile_index as u32, 1, 1, 1)
                        },
                        ColorMatcherAlgorithm::RgbWeighted { red_bias, green_bias, blue_bias } => {
                            matcher.find_nearest_color(tile, tile_index as u32, red_bias, green_bias, blue_bias)
                        },
                        ColorMatcherAlgorithm::Oklab => {
                            matcher.find_nearest_oklab_color(tile, tile_index as u32, 1.0, 1.0)
                        },
                        ColorMatcherAlgorithm::OklabWeighted { lightness_weight, chroma_weight } => {
                            matcher.find_nearest_oklab_color(tile, tile_index as u32, lightness_weight, chroma_weight)
                        },
                    };

                    if tx.send(frame_match).is_err() {
                        // The receiver was dropped, so stop working.
                        break;
                    }
                }
            });
        }

        drop(tx);

        Ok(rc)
    }

    fn find_nearest_color(&self, candidate: &ImageTile, tile_index: u32, red_bias: u8, green_bias: u8, blue_bias: u8) -> FrameMatch {
        let mut nearest_match = FrameMatch {
            tile_index,
            video_filename: String::new(),
            frame_index: 0,
            crop_resize: 100.0,
            crop_pos_x: 0.0,
            crop_pos_y: 0.0,
            is_flipped: false,
        };

        let mut smallest_distance = u64::MAX;
        
        for (video_filename, database) in &self.databases {
            let mut match_found_in_database = false;

            for frame in database.frames() {
                let allowed_crops = frame.crops.iter()
                    .filter(|c| self.allowed_crops.contains(&c.crop_level));

                for crop in allowed_crops {
                    // check non-flipped
                    let distance = self.check_distance(&crop.colors, candidate, red_bias, green_bias, blue_bias);

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
                    let flipped_distance = self.check_distance(crop.colors_flipped(), candidate, red_bias, green_bias, blue_bias);

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

    fn check_distance<'color, I>(&self, frame_colors: I, candidate: &ImageTile, red_bias: u8, green_bias: u8, blue_bias: u8) -> u64
    where
        I: IntoIterator<Item = &'color Color>,
    {
        let mut frame_colors = frame_colors.into_iter();
        let mut distance: u64 = 0;

        for image_color in &candidate.colors {
            let Some(frame_color) = frame_colors.next() else {
                return u64::MAX;
            };

            distance += frame_color.distance_weighted(
                image_color, red_bias, green_bias, blue_bias);
        }

        distance
    }

    fn find_nearest_oklab_color(&self, candidate: &ImageTile, tile_index: u32, lightness_weight: f32, chroma_weight: f32) -> FrameMatch {
        let mut nearest_match = FrameMatch {
            tile_index,
            video_filename: String::new(),
            frame_index: 0,
            crop_resize: 100.0,
            crop_pos_x: 0.0,
            crop_pos_y: 0.0,
            is_flipped: false,
        };

        let mut smallest_distance = f32::INFINITY;
        
        for (video_filename, database) in &self.databases {
            let mut match_found_in_database = false;

            for frame in database.frames() {
                let allowed_crops = frame.crops.iter()
                    .filter(|c| self.allowed_crops.contains(&c.crop_level));

                for crop in allowed_crops {
                    // check non-flipped
                    let distance = self.check_oklab_distance(&crop.oklab_colors, candidate, lightness_weight, chroma_weight);

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
                    let flipped_distance = self.check_oklab_distance(crop.oklab_colors_flipped(), candidate, lightness_weight, chroma_weight);

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

    fn check_oklab_distance<'color, I>(&self, frame_colors: I, candidate: &ImageTile, lightness_weight: f32, chroma_weight: f32) -> f32
    where
        I: IntoIterator<Item = &'color OklabColor>,
    {
        let mut frame_colors = frame_colors.into_iter();
        let mut distance: f32 = 0.0;

        for image_color in &candidate.oklab_colors {
            let Some(frame_color) = frame_colors.next() else {
                return f32::INFINITY;
            };

            distance += frame_color.distance_weighted(image_color, lightness_weight, chroma_weight);
        }

        distance
    }
}

#[derive(Debug, Clone)]
pub struct FrameMatch {
    pub tile_index: u32,

    pub video_filename: String,
    pub frame_index: u32,
    
    pub crop_resize: f32,
    pub crop_pos_x: f32,
    pub crop_pos_y: f32,
    pub is_flipped: bool,
}

#[derive(Debug, Clone)]
pub struct ImageTile {
    pub colors: Vec<Color>,
    pub oklab_colors: Vec<OklabColor>,
}
