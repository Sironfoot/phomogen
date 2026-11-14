use std::{collections::HashMap, sync::Arc};

use crate::{app::VideoColorIndexDatabase, ffmpeg::crops::CropLevel, frame_matcher::{FrameMatch, ImageTile, MatcherAlgorithm, OklabColor}};

pub struct OklabMatcher {
    pub lightness_weight: f32,
    pub chroma_weight: f32,
}

impl OklabMatcher {
    fn check_oklab_distance<'color, I>(&self, frame_colors: I, candidate: &ImageTile) -> f32
    where
        I: IntoIterator<Item = &'color OklabColor>,
    {
        let mut frame_colors = frame_colors.into_iter();
        let mut distance: f32 = 0.0;

        for image_color in &candidate.oklab_colors {
            let Some(frame_color) = frame_colors.next() else {
                return f32::INFINITY;
            };

            distance += frame_color.distance_weighted(image_color, self.lightness_weight, self.chroma_weight);
        }

        distance
    }
}

impl MatcherAlgorithm for OklabMatcher {
    fn find_nearest_frame(&self,
        candidate: &ImageTile,
        tile_index: u32,
        databases: &HashMap<String, Arc<VideoColorIndexDatabase>>,
        allowed_crops: &Vec<CropLevel>
    ) -> FrameMatch {

        let mut nearest_match = FrameMatch {
            tile_index,
            video_filename: String::new(),
            frame_index: 0,
            crop_resize: 100.0,
            crop_pos_x: 0,
            crop_pos_y: 0,
            is_flipped: false,
        };

        let mut smallest_distance = f32::INFINITY;
        
        for (video_filename, database) in databases {
            let mut match_found_in_database = false;
            let crops = &database.crops;

            for frame in database.frames() {
                let allowed_crops = frame.crops.iter()
                    .filter(|c| allowed_crops.contains(&c.crop_level));

                for frame_crop in allowed_crops {
                    let Some(crop) = crops.iter()
                        .find(|c| c.resize_percentage() as f32 == frame_crop.resize_percentage)
                    else { continue; };

                    let color_tile_width = crop.color_tiles_x() as u8;
                    let color_tile_height = crop.color_tiles_y() as u8;

                    for x in 0..color_tile_width {
                        for y in 0..color_tile_height {
                            // check non-flipped
                            let sub_grid = match frame_crop.extract_sub_grid_oklab(x, y, crop) {
                                Ok(colors) => colors,
                                Err(err) => panic!("Error: {}", err),
                            };

                            let distance = self.check_oklab_distance(sub_grid, candidate);

                            if distance < smallest_distance {
                                nearest_match.tile_index = tile_index;
                                nearest_match.frame_index =  frame.frame_index;
                                nearest_match.crop_resize = frame_crop.resize_percentage;
                                nearest_match.crop_pos_x = x;
                                nearest_match.crop_pos_y = y;
                                nearest_match.is_flipped = false;

                                smallest_distance = distance;
                                match_found_in_database = true;
                            }

                            let sub_grid_flipped = match frame_crop.extract_sub_grid_flipped_oklab(x, y, crop) {
                                Ok(colors) => colors,
                                Err(err) => panic!("Error: {}", err),
                            };

                            // check flipped
                            let flipped_distance = self.check_oklab_distance(sub_grid_flipped, candidate);

                            if flipped_distance < smallest_distance {
                                nearest_match.tile_index = tile_index;
                                nearest_match.frame_index =  frame.frame_index;
                                nearest_match.crop_resize = frame_crop.resize_percentage;
                                nearest_match.crop_pos_x = x;
                                nearest_match.crop_pos_y = y;
                                nearest_match.is_flipped = true;

                                smallest_distance = flipped_distance;
                                match_found_in_database = true;
                            }
                        }
                    }
                }
            }

            if match_found_in_database {
                nearest_match.video_filename = video_filename.clone();
            }
        }

        nearest_match
    }
}