use std::sync::Arc;

use crate::{app::{PrintSize, TileShape}, color_matcher::{FrameMatch, ImageTile}};

pub const MAX_TILES_PER_AXIS: u8 = 80;

#[derive(Debug)]
pub struct MosaicTilingOption {
    pub num_tiles_x: u8,
    pub num_tiles_y: u8,
    pub crop_percentage: f64,
    pub cropped_width: u32,
    pub cropped_height: u32,
    pub ideal_print_size: PrintSize,

    pub is_chosen: bool,
    pub image_tiles: Option<Arc<Vec<ImageTile>>>,
    pub matched_tiles: Option<Vec<FrameMatch>>,
}

impl MosaicTilingOption {
    pub fn new(num_tiles_x: u8, num_tiles_y: u8, crop_percentage: f64, cropped_width: u32, cropped_height: u32) -> Self {
        Self {
            num_tiles_x,
            num_tiles_y,
            crop_percentage,
            cropped_width,
            cropped_height,
            ideal_print_size: PrintSize::from_mosaic_tile_layout(num_tiles_x, num_tiles_y),
            is_chosen: false,
            image_tiles: None,
            matched_tiles: None,
        }
    }
    
    pub fn tiling_candidates(
        image_width: u32,
        image_height: u32,
        max_allowed_cropping_percentage: f64,
        tile_shape: &TileShape) -> Vec<Self> {

        if image_width == 0 || image_height == 0 {
            return Vec::new();
        }

        let mut candidates = Vec::new();

        let max_allowed_cropping_percentage = max_allowed_cropping_percentage.min(100.0);
        let (tile_aspect_width, tile_aspect_height) = tile_shape.aspect_dimensions();
        let image_area = image_width * image_height;

        for mosaic_tile_x in 1..=MAX_TILES_PER_AXIS {
            for mosaic_tile_y in 1..=MAX_TILES_PER_AXIS {
                let grid_width = tile_aspect_width as u32 * mosaic_tile_x as u32;
                let grid_height = tile_aspect_height as u32 * mosaic_tile_y as u32;

                let grid_cross = grid_width * image_height;
                let image_cross = grid_height * image_width;

                let (cropped_width, cropped_height) = if grid_cross > image_cross {
                    // The grid is wider, so preserve the width and crop vertically.
                    let cropped_height = Self::divide_round(image_width * grid_height, grid_width).min(image_height);

                    (image_width, cropped_height)
                }
                else if grid_cross < image_cross {
                    // The grid is narrower, so preserve the height and crop horizontally.
                    let cropped_width = Self::divide_round(image_height * grid_width, grid_height).min(image_width);

                    (cropped_width, image_height)
                }
                else {
                    (image_width, image_height)
                };

                let cropped_area = cropped_width * cropped_height;
                let crop_percentage = image_area.abs_diff(cropped_area) as f64 / image_area as f64 * 100.0;

                if crop_percentage <= max_allowed_cropping_percentage {
                    candidates.push(Self::new(
                        mosaic_tile_x,
                        mosaic_tile_y,
                        crop_percentage,
                        cropped_width,
                        cropped_height));
                }
            }
        }

        candidates
    }

    fn divide_round(numerator: u32, denominator: u32) -> u32 {
        (numerator + denominator / 2) / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_candidates_16x9() {
        let width: u32 = 1920;
        let height: u32 = 1080;

        let expected: Vec<(u8, u8)> = (1..=80).map(|x| (x, x)).collect();

        let candidates = MosaicTilingOption::tiling_candidates(
            width, height, 0.0, &TileShape::Landscape16x9);

        for (i, candidate) in candidates.iter().enumerate() {
            let (expected_tiles_x, expected_tiles_y) = expected[i];

            assert_eq!(expected_tiles_x, candidate.num_tiles_x);
            assert_eq!(expected_tiles_y, candidate.num_tiles_y);
            assert_eq!(0.0, candidate.crop_percentage);
            assert_eq!(width, candidate.cropped_width);
            assert_eq!(height, candidate.cropped_height);
        }
    }

    #[test]
    fn tiling_candidates_9x16() {
        let width: u32 = 1080;
        let height: u32 = 1920;

        let expected: Vec<(u8, u8)> = (1..=80).map(|x| (x, x)).collect();

        let candidates = MosaicTilingOption::tiling_candidates(
            width, height, 0.0, &TileShape::Portrait9x16);

        for (i, candidate) in candidates.iter().enumerate() {
            let (expected_tiles_x, expected_tiles_y) = expected[i];

            assert_eq!(expected_tiles_x, candidate.num_tiles_x);
            assert_eq!(expected_tiles_y, candidate.num_tiles_y);
            assert_eq!(0.0, candidate.crop_percentage);
            assert_eq!(width, candidate.cropped_width);
            assert_eq!(height, candidate.cropped_height);
        }
    }

    #[test]
    fn tiling_candidates_1x1() {
        let width: u32 = 1920;
        let height: u32 = 1920;

        let expected: Vec<(u8, u8)> = (1..=80).map(|x| (x, x)).collect();

        let candidates = MosaicTilingOption::tiling_candidates(
            width, height, 0.0, &TileShape::Square);

        for (i, candidate) in candidates.iter().enumerate() {
            let (expected_tiles_x, expected_tiles_y) = expected[i];

            assert_eq!(expected_tiles_x, candidate.num_tiles_x);
            assert_eq!(expected_tiles_y, candidate.num_tiles_y);
            assert_eq!(0.0, candidate.crop_percentage);
            assert_eq!(width, candidate.cropped_width);
            assert_eq!(height, candidate.cropped_height);
        }
    }
}