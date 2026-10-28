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
            crop_pos_x: 0.0,
            crop_pos_y: 0.0,
            is_flipped: false,
        };

        let mut smallest_distance = f32::INFINITY;
        
        for (video_filename, database) in databases {
            let mut match_found_in_database = false;

            for frame in database.frames() {
                let allowed_crops = frame.crops.iter()
                    .filter(|c| allowed_crops.contains(&c.crop_level));

                for crop in allowed_crops {
                    // check non-flipped
                    let distance = self.check_oklab_distance(&crop.oklab_colors, candidate);

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
                    let flipped_distance = self.check_oklab_distance(crop.oklab_colors_flipped(), candidate);

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
}