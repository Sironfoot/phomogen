use std::{cmp};
use anyhow::Result;

use crate::ffmpeg::AspectRatio;

const RATIO_16_9 : f64 = 16.0 / 9.0;

pub struct CropSetting {
    pub resize_percentage: f64,
    pub pos_x_percentage: f64,
    pub pos_y_percentage: f64,

    pub tiles: Vec<CropTile>,
}

pub struct CropTile {
    pub start_x: u32,
    pub end_x: u32,
    pub start_y: u32,
    pub end_y: u32,

    pub total_pixels: u32,
}

impl CropSetting {
    fn new(resize: f64, pos_x: f64, pos_y: f64, frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Self {
        let cropped_width = f64::round((frame_width as f64 / 100.0) * resize) as u32;
        let cropped_height = f64::round(cropped_width as f64 / RATIO_16_9) as u32;

        let crop_start_x = f64::round((frame_width as f64 / 100.0) * pos_x) as u32;
        let crop_start_y = f64::round((frame_height as f64 / 100.0) * pos_y) as u32;

        let cropped_grid_width = f64::round(cropped_width as f64 / tiles_x as f64) as u32;
        let cropped_grid_height = f64::round(cropped_height as f64 / tiles_y as f64) as u32;

        let mut tiles: Vec<CropTile> = Vec::with_capacity((tiles_x * tiles_y) as usize);

        for grid_y in 0..tiles_y {
            for grid_x in 0..tiles_x {
                let start_x = crop_start_x + (cropped_grid_width * grid_x);
                let end_x = cmp::min(start_x + cropped_grid_width, frame_width);

                let start_y = crop_start_y + (cropped_grid_height * grid_y);
                let end_y = cmp::min(start_y + cropped_grid_height, frame_height);

                let tile_width = end_x - start_x;
                let tile_height = end_y - start_y;
                let total_pixels = tile_width * tile_height;

                tiles.push(CropTile { start_x, end_x, start_y, end_y, total_pixels });
            }
        }

        Self {
            resize_percentage: resize,
            pos_x_percentage: pos_x,
            pos_y_percentage: pos_y,

            tiles: tiles,
        }
    }

    pub fn all_crops(frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Result<Vec<Self>> {
        let aspect_ratio = AspectRatio::new(frame_width, frame_height);

        match aspect_ratio {
            AspectRatio::Landscape16x9 => {
                Ok(CropSetting::get_16x9(frame_width, frame_height, tiles_x, tiles_y))
            },
            _ => {
                Err(anyhow::format_err!("`{aspect_ratio}` aspect ratio videos aren't supported"))
            }
        }
    }

    fn get_16x9(frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Vec<Self> {
        vec![
            // full frame
            Self::new(100.0, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y), // full frame

            // 50% crops
            Self::new(50.0, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),   // top left
            Self::new(50.0, 25.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),  // top
            Self::new(50.0, 50.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),  // top right
            Self::new(50.0, 0.0, 25.0, frame_width, frame_height, tiles_x, tiles_y),  // left
            Self::new(50.0, 25.0, 25.0, frame_width, frame_height, tiles_x, tiles_y), // center
            Self::new(50.0, 50.0, 25.0, frame_width, frame_height, tiles_x, tiles_y), // right
            Self::new(50.0, 0.0, 50.0, frame_width, frame_height, tiles_x, tiles_y),  // bottom left
            Self::new(50.0, 25.0, 50.0, frame_width, frame_height, tiles_x, tiles_y), // bottom
            Self::new(50.0, 50.0, 50.0, frame_width, frame_height, tiles_x, tiles_y), // bottom right

            // 50% inner crops
            Self::new(50.0, 12.5, 12.5, frame_width, frame_height, tiles_x, tiles_y), // inner top left
            Self::new(50.0, 37.5, 12.5, frame_width, frame_height, tiles_x, tiles_y), // inner top right
            Self::new(50.0, 12.5, 37.5, frame_width, frame_height, tiles_x, tiles_y), // inner bottom left
            Self::new(50.0, 37.5, 37.5, frame_width, frame_height, tiles_x, tiles_y), // inner bottom right

            // 66.666% crops
            Self::new(66.666, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),       // top left
            Self::new(66.666, 16.666, 0.0, frame_width, frame_height, tiles_x, tiles_y),    // top
            Self::new(66.666, 33.333, 0.0, frame_width, frame_height, tiles_x, tiles_y),    // top right
            Self::new(66.666, 0.0, 16.666, frame_width, frame_height, tiles_x, tiles_y),    // left
            Self::new(66.666, 16.666, 16.666, frame_width, frame_height, tiles_x, tiles_y), // center
            Self::new(66.666, 33.333, 16.666, frame_width, frame_height, tiles_x, tiles_y), // right
            Self::new(66.666, 0.0, 33.333, frame_width, frame_height, tiles_x, tiles_y),    // bottom left
            Self::new(66.666, 16.666, 33.333, frame_width, frame_height, tiles_x, tiles_y), // bottom
            Self::new(66.666, 33.333, 33.333, frame_width, frame_height, tiles_x, tiles_y), // bottom right
        ]
    }
}