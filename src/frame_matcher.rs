mod color;
pub use color::Color;

mod frame_match;
pub use frame_match::FrameMatch;

mod frame_matcher_algorithm;
pub use frame_matcher_algorithm::FrameMatcherAlgorithm;

mod image_tile;
pub use image_tile::ImageTile;

mod oklab_color;
pub use oklab_color::OklabColor;

mod rgb_matcher;
use rgb_matcher::RgbMatcher;
mod oklab_matcher;
use oklab_matcher::OklabMatcher;

use std::{collections::HashMap, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver}}, thread::{self}};

use anyhow::Result;

use crate::{app::VideoColorIndexDatabase, ffmpeg::crops::CropLevel};

pub trait MatcherAlgorithm: Send {
    fn find_nearest_frame(&self,
        candidate: &ImageTile,
        tile_index: u32,
        databases: &HashMap<String, Arc<VideoColorIndexDatabase>>,
        allowed_crops: &Vec<CropLevel>) -> FrameMatch;
}

pub struct FrameMatcher {
    pub mosaic_tiles_x: u32,
    pub mosaic_tiles_y: u32,

    algorithm: FrameMatcherAlgorithm,

    num_workers: u32,
    allowed_crops: Vec<CropLevel>,

    databases: HashMap<String, Arc<VideoColorIndexDatabase>>,
}

impl FrameMatcher {
    pub fn new(mosaic_tiles_x: u32, mosaic_tiles_y: u32, allowed_crops: &[CropLevel]) -> Self {
        Self {
            mosaic_tiles_x,
            mosaic_tiles_y,
            algorithm: FrameMatcherAlgorithm::Oklab,
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

    pub fn algorithm(&self) -> &FrameMatcherAlgorithm {
        &self.algorithm
    }

    pub fn set_algorithm(&mut self, algorithm: FrameMatcherAlgorithm) {
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

            let matcher_algorithm: Box<dyn MatcherAlgorithm> = match algorithm {
                FrameMatcherAlgorithm::Rgb => {
                    Box::new(RgbMatcher { red_bias: 1, green_bias: 1, blue_bias: 1 })
                },
                FrameMatcherAlgorithm::RgbWeighted { red_bias, green_bias, blue_bias } => {
                    Box::new(RgbMatcher { red_bias, green_bias, blue_bias })
                },
                FrameMatcherAlgorithm::Oklab => {
                    Box::new(OklabMatcher { lightness_weight: 1.0, chroma_weight: 1.0 })
                },
                FrameMatcherAlgorithm::OklabWeighted { lightness_weight, chroma_weight } => {
                    Box::new(OklabMatcher { lightness_weight, chroma_weight })
                },
            };

            thread::spawn(move || {
                loop {
                    let tile_index = next_tile_index.fetch_add(1, Ordering::Relaxed);

                    // stop the thread when no more tiles left
                    if tile_index >= num_mosaic_tiles {
                        break;
                    }

                    let tile = &image_tiles[tile_index];
                    let frame_match = matcher_algorithm.find_nearest_frame(tile, tile_index as u32, &matcher.databases, &matcher.allowed_crops);

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
}