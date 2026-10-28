use crate::{ffmpeg::crops::CropLevel, frame_matcher::{Color, OklabColor}};

#[derive(Debug)]
pub struct FrameCrop {
    pub resize_percentage: f32,
    pub pos_x_percentage: f32,
    pub pos_y_percentage: f32,
    pub crop_level: CropLevel,

    color_tiles_x: u8,
    pub colors: Vec<Color>,
    pub oklab_colors: Vec<OklabColor>,
}

impl FrameCrop {
    pub fn init(color_tiles_x: u8, resize_percentage: f32, pos_x_percentage: f32, pos_y_percentage: f32, crop_level: CropLevel) -> FrameCrop {
        FrameCrop {
            resize_percentage,
            pos_x_percentage,
            pos_y_percentage,
            crop_level,
            color_tiles_x: color_tiles_x,
            colors: vec![],
            oklab_colors: vec![],
        }
    }

    pub fn colors_flipped(&self) -> impl Iterator<Item = &Color> + '_ {
        self.colors
            .chunks_exact(self.color_tiles_x as usize)
            .flat_map(|row| row.iter().rev())
    }

    pub fn oklab_colors_flipped(&self) -> impl Iterator<Item = &OklabColor> + '_ {
        self.oklab_colors
            .chunks_exact(self.color_tiles_x as usize)
            .flat_map(|row| row.iter().rev())
    }
}