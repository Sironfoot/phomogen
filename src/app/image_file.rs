use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};

use crate::app::{TileShape, mosaic_tiling_option::{MosaicTilingOption, MAX_TILES_PER_AXIS}};
use crate::color_matcher::{FrameMatch, ImageTile};
use crate::images::PreviewImage;

pub struct ImageFile {
    pub file_name: String,
    pub full_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: super::ImageType,
    pub preview: Option<PreviewImage>, 
    pub is_chosen: bool,

    pub image_tiles: Option<Arc<Vec<ImageTile>>>,

    pub matched_tiles: Option<Vec<FrameMatch>>,

    pub tiling_options: HashMap<TileShape, Vec<MosaicTilingOption>>,
    pub selected_tiling_option_index: usize,
}

impl ImageFile {
    pub fn new(file_name: &str, full_path: &Path, width: u32, height: u32, format: super::ImageType) -> ImageFile {
        let tile_shapes = vec![
            TileShape::Landscape16x9, 
            TileShape::Square,
            TileShape::Portrait9x16
        ];

        let mut tiling_options: HashMap<TileShape, Vec<MosaicTilingOption>> = HashMap::with_capacity(tile_shapes.len());
        
        for tile_shape in tile_shapes {
            let tiling_candidates = MosaicTilingOption::tiling_candidates(
                width, height, 0.9, &tile_shape);
            let narrowwed_candidates = Self::narrow_down_candidates(
                &tiling_candidates, width, height, &tile_shape);

            tiling_options.insert(tile_shape, narrowwed_candidates);
        }

        ImageFile {
            file_name: String::from(file_name),
            full_path: PathBuf::from(full_path),
            width,
            height,
            format,
            preview: None,
            is_chosen: false,
            image_tiles: None,
            matched_tiles: None,
            tiling_options: tiling_options,
            selected_tiling_option_index: 0,
        }
    }

    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }

    fn narrow_down_candidates(candidates: &[MosaicTilingOption], image_width: u32, image_height: u32, tile_shape: &TileShape) -> Vec<MosaicTilingOption> {
        const MAX_CANDIDATES: usize = 20;

        let total_candidates = candidates.len();
        
        if total_candidates < MAX_CANDIDATES {
            return candidates.to_vec();
        }

        let mut narrowed_candidates: Vec<MosaicTilingOption> = Vec::with_capacity(MAX_CANDIDATES);

        // image shape perfectly (or nearly perfectly) matches tile shape, expecting 1x1, 2x2, 3x3, 4x4.....80x80
        let image_aspect_ratio = image_width as f64 / image_height as f64;
        let all_tiles_active = total_candidates as u8 == MAX_TILES_PER_AXIS;

        if image_aspect_ratio == tile_shape.aspect_ratio() || all_tiles_active {
            for candidate in candidates.iter() {

                // output 1x1, 5x5, 10x10, 15x15, 20x20, 25x25, 30x30....80x80
                match candidate.num_tiles_x {
                    1 => narrowed_candidates.push(candidate.clone()),
                    x if x % 5 == 0 =>  narrowed_candidates.push(candidate.clone()),
                    _ => {},
                }
            }

            return narrowed_candidates;
        }

        let skip_count = (total_candidates as f64 / MAX_CANDIDATES as f64).round() as usize;

        // all tile arrangements fit perfectly
        let all_candidates_have_no_crops = candidates.iter().all(|c| c.crop_percentage == 0.0);
        if all_candidates_have_no_crops {
            for (i, candidate) in candidates.iter().enumerate() {
                if i % skip_count == 0 {
                    narrowed_candidates.push(candidate.clone());
                }
            }
        }
        else {
            for (i, candidate) in candidates.iter().enumerate() {
                // always include first
                if i == 0 {
                    narrowed_candidates.push(candidate.clone());
                    continue;
                }

                if i % skip_count == 0 {
                    let prev_candidate = &candidates[i-1];
                    let next_candidate = candidates.get(i + 1);

                    let mut compare: Vec<&MosaicTilingOption> = vec![prev_candidate, candidate];
                    if let Some(next_candidate) = next_candidate {
                        compare.push(next_candidate);
                    }

                    let with_smallest_cropping = compare.iter()
                        .min_by_key(|c| (c.crop_percentage * 1000.0).round() as u32);
                    if let Some(with_smallest_cropping) = with_smallest_cropping {
                        if !narrowed_candidates.contains(*with_smallest_cropping) {
                            narrowed_candidates.push(with_smallest_cropping.to_owned().clone());
                        }
                        else {
                            narrowed_candidates.push(candidate.clone());
                        }
                    }
                }
            }
        }

        narrowed_candidates
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ImageType {
    BMP,
    JPEG,
    PNG,
    WEBP,
    TIFF,
}