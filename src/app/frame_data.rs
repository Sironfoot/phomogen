use crate::ffmpeg::crops::CropLevel;

pub struct VideoColorIndexDatabase {
    pub tiles_x: u32,
    pub tiles_y: u32,

    frames: Vec<FrameData>,
}

impl VideoColorIndexDatabase {
    pub fn new(tiles_x: u32, tiles_y: u32, frames: Vec<FrameData>) -> VideoColorIndexDatabase {
        VideoColorIndexDatabase {
            tiles_x,
            tiles_y,
            frames,
        }
    }

    pub fn frames(&self) -> impl Iterator<Item = &FrameData> + '_ { // avoiding memory allocation for large videos
        self.frames.iter()
    }

    pub fn total_frames(&self) -> u64 {
        self.frames.len() as u64
    }
}

#[derive(Debug)]
pub struct FrameData {
    pub frame_index: u32,
    pub crops: Vec<FrameCrop>,
}

impl FrameData {
    pub fn new(frame_index: u32) -> FrameData {
        FrameData {
            frame_index,
            crops: vec![],
        }
    }
}

#[derive(Debug)]
pub struct FrameCrop {
    pub resize_percentage: f64,
    pub pos_x_percentage: f64,
    pub pos_y_percentage: f64,
    pub crop_level: CropLevel,

    tiles_x: usize,
    pub colors: Vec<Color>,
}

impl FrameCrop {
    pub fn init(tiles_x: u32, resize_percentage: f64, pos_x_percentage: f64, pos_y_percentage: f64, crop_level: CropLevel) -> FrameCrop {
        FrameCrop {
            resize_percentage,
            pos_x_percentage,
            pos_y_percentage,
            crop_level,
            tiles_x: tiles_x as usize,
            colors: vec![]
        }
    }

    pub fn colors_flipped(&self) -> impl Iterator<Item = &Color> + '_ {
        self.colors
            .chunks_exact(self.tiles_x)
            .flat_map(|row| row.iter().rev())
    }
}

#[derive(Debug, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn darken(&self, percentage: u8) -> Color{
        let percentage = percentage.min(100) as f32;

        Color {
            r: self.r - ((self.r as f32 / 100.0) * percentage).round() as u8,
            g: self.g - ((self.g as f32 / 100.0) * percentage).round() as u8,
            b: self.b - ((self.b as f32 / 100.0) * percentage).round() as u8,
        }
    }

    pub fn lighten(&self, percentage: u8) -> Color{
        let percentage = percentage.min(100) as f32;

        Color {
            r: self.r + ((self.r as f32 / 100.0) * percentage).round() as u8,
            g: self.g + ((self.g as f32 / 100.0) * percentage).round() as u8,
            b: self.b + ((self.b as f32 / 100.0) * percentage).round() as u8,
        }
    }

    pub fn blend_toward(&self, toward: &Color, percentage: u8) -> Color {
        let percentage = percentage.min(100) as f32;

        Color {
            r: Self::blend_channel(self.r, toward.r, percentage),
            g: Self::blend_channel(self.g, toward.g, percentage),
            b: Self::blend_channel(self.b, toward.b, percentage),
        }
    }

    fn blend_channel(base: u8, toward: u8, percentage: f32) -> u8 {
        ((base as f32 * (100.0 - percentage) + toward as f32 * percentage) / 100.0).round() as u8
    }

    pub fn to_ratatui_color(&self) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(self.r, self.g, self.b)
    }
}