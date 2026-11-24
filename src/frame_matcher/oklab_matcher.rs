use std::{collections::HashMap, sync::Arc};

use crate::{app::VideoColorIndexDatabase, ffmpeg::CropLevel, frame_matcher::{FrameMatch, ImageTile, MatcherAlgorithm, OklabColor}};

pub struct OklabMatcher {
    pub lightness_weight: f32,
    pub chroma_weight: f32,
}

impl OklabMatcher {
    fn check_distance<'color, I>(&self, frame_colors: I, oklab_colors: &[OklabColor], current_smallest_distance: f32) -> f32
    where
        I: IntoIterator<Item = &'color OklabColor>,
    {
        let mut frame_colors = frame_colors.into_iter();
        let mut distance: f32 = 0.0;

        for image_color in oklab_colors {
            let Some(frame_color) = frame_colors.next() else {
                return f32::INFINITY;
            };

            distance += frame_color.distance_weighted(image_color, self.lightness_weight, self.chroma_weight);

            // optimisation: bail out early if current accumulated
            // distance is more than the current smallest distaqnce
            if distance >= current_smallest_distance {
                return f32::INFINITY;
            }
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

        let candidate_colors = match &candidate.oklab_colors {
            Some(colors) => colors,
            None => {
                panic!("canidate has no Oklab colors set")
            },
        };

        let mut nearest_match = FrameMatch {
            tile_index,
            video_filename: String::new(),
            frame_index: 0,
            crop_resize: 100,
            crop_pos_x: 0,
            crop_pos_y: 0,
            is_flipped: false,
        };

        let mut smallest_distance = f32::INFINITY;
        
        // loop through each video
        for (video_filename, database) in databases {
            let mut match_found_in_database = false;
            let crops = &database.crops;

            // loop through each frame in that video
            for frame in database.frames() {
                
                // loop through each crop level for that frame (100% - full frame, 60% frame crop etc.)
                for frame_crop in frame.crops.iter() {
                    let Some(crop) = crops.iter()
                        .find(|c| c.resize_percentage() == frame_crop.resize_percentage && allowed_crops.contains(&c.crop_level))
                    else { continue; };

                    for y in 0..crop.tile_down_shift() {
                        for x in 0..crop.tile_right_shift() {
                            // check non-flipped
                            let sub_grid = match frame_crop.extract_sub_grid_oklab(x, y, crop) {
                                Ok(colors) => colors,
                                Err(err) => panic!("Error: {}", err),
                            };

                            let distance = self.check_distance(sub_grid, candidate_colors, smallest_distance);

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

                            // check flipped
                            let sub_grid_flipped = match frame_crop.extract_sub_grid_flipped_oklab(x, y, crop) {
                                Ok(colors) => colors,
                                Err(err) => panic!("Error: {}", err),
                            };

                            let flipped_distance = self.check_distance(sub_grid_flipped, candidate_colors, smallest_distance);

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