use std::borrow::Cow;

use image::{Rgb, RgbImage, imageops};
use anyhow::Result;

const DEFAULT_BLEND_WIDTH: u32 = 12;
const DEFAULT_SAMPLE_LENGTH: u32 = 15;

pub struct TileBlender {
    row: u32,
    col: u32,

    mosaic_tiles_x: u32,
    mosaic_tiles_y: u32,

    blend_width: u32,

    sample_width: u32,
    sample_height: u32,
}

impl TileBlender {
    pub fn new(row: u32, col: u32, mosaic_tiles_x: u32, mosaic_tiles_y: u32) -> TileBlender {
        TileBlender {
            row,
            col,
            mosaic_tiles_x,
            mosaic_tiles_y,
            blend_width: DEFAULT_BLEND_WIDTH,
            sample_width: DEFAULT_SAMPLE_LENGTH,
            sample_height: DEFAULT_SAMPLE_LENGTH,
        }
    }

    pub fn find_top(&self) -> Option<(u32, u32)> {
        if self.row == 0 {
            return None;
        }

        Some((self.row - 1, self.col))
    }

    pub fn find_right(&self) -> Option<(u32, u32)> {
        if self.col == (self.mosaic_tiles_x - 1) {
            return None;
        }

        Some((self.row, self.col + 1))
    }

    pub fn find_bottom(&self) -> Option<(u32, u32)> {
        if self.row == (self.mosaic_tiles_y - 1) {
            return None;
        }

        Some((self.row + 1, self.col))
    }

    pub fn find_left(&self) -> Option<(u32, u32)> {
        if self.col == 0 {
            return None;
        }

        Some((self.row, self.col - 1))
    }

    pub fn blend_image(&self,
        center_image: &mut RgbImage,
        top_image: Option<&RgbImage>,
        right_image: Option<&RgbImage>,
        bottom_image: Option<&RgbImage>, 
        left_image: Option<&RgbImage>) -> Result<()> {

        let width = center_image.width();
        let height = center_image.height();

        if let Some(top_image) = top_image {
            let top_image = if top_image.dimensions() != (width, height) {
                Cow::Owned(imageops::resize(top_image, width, height, imageops::FilterType::Triangle))
            }
            else {
                Cow::Borrowed(top_image)
            };

            for x in 0..width {
                let top_average = average_bottom_edge(&top_image, x, self.sample_height);
                let center_average = average_top_edge(&center_image, x, self.sample_height);

                let seam_colour = Rgb([
                    ((top_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                    ((top_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                    ((top_average[2] as u16 + center_average[2] as u16) / 2) as u8,
                ]);

                for i in 0..self.blend_width {
                    // 0.0 at the outside of the blend area,
                    // 1.0 at the actual seam.
                    let t = (i + 1) as f32 / self.blend_width as f32;

                    // Smoothstep:
                    // gives a softer transition than a simple linear interpolation.
                    let t = t * t * (3.0 - 2.0 * t);
  
                    // Reverse i so that blending is strongest at x = 0.
                    let y2 = self.blend_width - 1 - i;

                    let original = *center_image.get_pixel(x, y2);
                    let blended = blend_pixel(original, seam_colour, t);
                    center_image.put_pixel(x, y2, blended);
                }
            }
        }

        if let Some(right_image) = right_image {
            let right_image = if right_image.dimensions() != (width, height) {
                Cow::Owned(imageops::resize(right_image, width, height, imageops::FilterType::Triangle))
            }
            else {
                Cow::Borrowed(right_image)
            };

            for y in 0..height {
                let center_average = average_right_edge(&center_image, y, self.sample_width);
                let right_average = average_left_edge(&right_image, y, self.sample_width);

                let seam_colour = Rgb([
                    ((center_average[0] as u16 + right_average[0] as u16) / 2) as u8,
                    ((center_average[1] as u16 + right_average[1] as u16) / 2) as u8,
                    ((center_average[2] as u16 + right_average[2] as u16) / 2) as u8,
                ]);

                for i in 0..self.blend_width {
                    // 0.0 at the outside of the blend area,
                    // 1.0 at the actual seam.
                    let t = (i + 1) as f32 / self.blend_width as f32;

                    // Smoothstep:
                    // gives a softer transition than a simple linear interpolation.
                    let t = t * t * (3.0 - 2.0 * t);

                    let x1 = width - self.blend_width + i;

                    let original = *center_image.get_pixel(x1, y);
                    let blended = blend_pixel(original, seam_colour, t);
                    center_image.put_pixel(x1, y, blended);
                }
            }
        }

        if let Some(bottom_image) = bottom_image {
            let bottom_image = if bottom_image.dimensions() != (width, height) {
                Cow::Owned(imageops::resize(bottom_image, width, height, imageops::FilterType::Triangle))
            }
            else {
                Cow::Borrowed(bottom_image)
            };

            for x in 0..width {
                let center_average = average_bottom_edge(&center_image, x, self.sample_height);
                let bottom_average = average_top_edge(&bottom_image, x, self.sample_height);

                let seam_colour = Rgb([
                    ((bottom_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                    ((bottom_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                    ((bottom_average[2] as u16 + center_average[2] as u16) / 2) as u8,
                ]);

                for i in 0..self.blend_width {
                    // 0.0 at the outside of the blend area,
                    // 1.0 at the actual seam.
                    let t = (i + 1) as f32 / self.blend_width as f32;

                    // Smoothstep:
                    // gives a softer transition than a simple linear interpolation.
                    let t = t * t * (3.0 - 2.0 * t);

                    let y1 = height - self.blend_width + i;

                    let original = *center_image.get_pixel(x, y1);
                    let blended = blend_pixel(original, seam_colour, t);
                    center_image.put_pixel(x, y1, blended);
                }
            }
        }

        if let Some(left_image) = left_image {
            let left_image = if left_image.dimensions() != (width, height) {
                Cow::Owned(imageops::resize(left_image, width, height, imageops::FilterType::Triangle))
            }
            else {
                Cow::Borrowed(left_image)
            };

            for y in 0..height {
                let left_average = average_right_edge(&left_image, y, self.sample_width);
                let center_average = average_left_edge(&center_image, y, self.sample_width);

                let seam_colour = Rgb([
                    ((left_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                    ((left_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                    ((left_average[2] as u16 + center_average[2] as u16) / 2) as u8,
                ]);

                for i in 0..self.blend_width {
                    // 0.0 at the outside of the blend area,
                    // 1.0 at the actual seam.
                    let t = (i + 1) as f32 / self.blend_width as f32;

                    // Smoothstep:
                    // gives a softer transition than a simple linear interpolation.
                    let t = t * t * (3.0 - 2.0 * t);

                    // Reverse i so that blending is strongest at x = 0.
                    let x2 = self.blend_width - 1 - i;

                    let original = *center_image.get_pixel(x2, y);
                    let blended = blend_pixel(original, seam_colour, t);
                    center_image.put_pixel(x2, y, blended);
                }
            }
        }

        Ok(())
    }
}


fn blend_pixel(original: Rgb<u8>, target: Rgb<u8>, amount: f32) -> Rgb<u8> {
    Rgb([
        lerp(original[0], target[0], amount),
        lerp(original[1], target[1], amount),
        lerp(original[2], target[2], amount),
    ])
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn average_top_edge(image: &RgbImage, x: u32, sample_height: u32) -> Rgb<u8> {
    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for y in 0..sample_height {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_height) as u8,
        (g / sample_height) as u8,
        (b / sample_height) as u8,
    ])
}

fn average_right_edge(image: &RgbImage, y: u32, sample_width: u32) -> Rgb<u8> {
    let width = image.width();

    let start_x = width - sample_width;

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for x in start_x..width {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_width) as u8,
        (g / sample_width) as u8,
        (b / sample_width) as u8,
    ])
}

fn average_bottom_edge(image: &RgbImage, x: u32, sample_height: u32) -> Rgb<u8> {
    let height = image.height();

    let start_y = height - sample_height;

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for y in start_y..height {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_height) as u8,
        (g / sample_height) as u8,
        (b / sample_height) as u8,
    ])
}

fn average_left_edge(image: &RgbImage, y: u32, sample_width: u32) -> Rgb<u8> {
    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for x in 0..sample_width {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_width) as u8,
        (g / sample_width) as u8,
        (b / sample_width) as u8,
    ])
}