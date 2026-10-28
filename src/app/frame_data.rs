use image::Rgba;

use crate::ffmpeg::crops::CropLevel;

pub struct VideoColorIndexDatabase {
    pub color_tiles_x: u32,
    pub color_tiles_y: u32,

    frames: Vec<FrameData>,
}

impl VideoColorIndexDatabase {
    pub fn new(color_tiles_x: u32, color_tiles_y: u32, frames: Vec<FrameData>) -> VideoColorIndexDatabase {
        VideoColorIndexDatabase {
            color_tiles_x,
            color_tiles_y,
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

#[derive(Debug, PartialEq, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn from_rgba(rgba: Rgba<u8>) -> Self {
        let [r, g, b, _] = rgba.0;
        Self { r, g, b }
    }

    pub fn distance(&self, other: &Self) -> u64 {
        self.distance_weighted(other, 1, 1, 1)
    }

    pub fn distance_weighted(&self, other: &Self, red_bias: u8, green_bias: u8, blue_bias: u8) -> u64 {
        let red_dist = self.r.abs_diff(other.r) as u64;
        let green_dist = self.g.abs_diff(other.g) as u64;
        let blue_dist = self.b.abs_diff(other.b) as u64;

        (red_bias as u64 * red_dist * red_dist) +
        (green_bias as u64 * green_dist * green_dist) +
        (blue_bias as u64 * blue_dist * blue_dist)
    }

    pub fn darken(&self, percentage: u8) -> Self {
        let percentage = percentage.min(100) as f32;

        Self {
            r: self.r - ((self.r as f32 / 100.0) * percentage).round() as u8,
            g: self.g - ((self.g as f32 / 100.0) * percentage).round() as u8,
            b: self.b - ((self.b as f32 / 100.0) * percentage).round() as u8,
        }
    }

    pub fn lighten(&self, percentage: u8) -> Self {
        let percentage = percentage.min(100) as f32;

        Self {
            r: self.r + ((self.r as f32 / 100.0) * percentage).round() as u8,
            g: self.g + ((self.g as f32 / 100.0) * percentage).round() as u8,
            b: self.b + ((self.b as f32 / 100.0) * percentage).round() as u8,
        }
    }

    pub fn blend_toward(&self, toward: &Self, percentage: u8) -> Self {
        let percentage = percentage.min(100) as f32;

        Self {
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

    pub fn to_rgba_color(&self) -> image::Rgba<u8> {
        image::Rgba([self.r, self.g, self.b, 255])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OklabColor {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl OklabColor {
    pub fn from_srgb(red: u8, green: u8, blue: u8) -> Self {
        let red = Self::srgb_channel_to_linear(red);
        let green = Self::srgb_channel_to_linear(green);
        let blue = Self::srgb_channel_to_linear(blue);

         // Linear RGB → LMS.
        let l = 0.412_221_46 * red
            + 0.536_332_55 * green
            + 0.051_445_995 * blue;

        let m = 0.211_903_5 * red
            + 0.680_699_5 * green
            + 0.107_396_96 * blue;

        let s = 0.088_302_46 * red
            + 0.281_718_85 * green
            + 0.629_978_7 * blue;

        let l_root = l.cbrt();
        let m_root = m.cbrt();
        let s_root = s.cbrt();

        // Nonlinear LMS → Oklab.
        Self {
            l: 0.210_454_26 * l_root
                + 0.793_617_8 * m_root
                - 0.004_072_047 * s_root,

            a: 1.977_998_5 * l_root
                - 2.428_592_2 * m_root
                + 0.450_593_7 * s_root,

            b: 0.025_904_037 * l_root
                + 0.782_771_77 * m_root
                - 0.808_675_77 * s_root,
        }
    }

    pub fn from_color(color: &Color) -> Self {
        Self::from_srgb(color.r, color.g, color.b)
    }

    pub fn distance(&self, other: &Self) -> f32 {
        self.distance_weighted(other, 1.0, 1.0)
    }

    pub fn distance_weighted(&self, other: &Self, lightness_weight: f32, chroma_weight: f32) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;

        (lightness_weight * dl * dl) + (chroma_weight * (da * da + db * db))
    }

    fn srgb_channel_to_linear(channel: u8) -> f32 {
        let value = channel as f32 / 255.0;

        if value <= 0.04045 {
            return value / 12.92;
        }

        ((value + 0.055) / 1.055).powf(2.4)
    }
}