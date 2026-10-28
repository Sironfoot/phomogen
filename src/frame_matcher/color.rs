use image::Rgba;

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