use crate::ffmpeg::{
    color_extractor::FrameColorExtractionAlgorithm,
    crops::CropSetting
};

use super::compute_output_buffer_size;

use std::fmt::Write;
use anyhow::Result;

const BYTES_PER_PIXEL: u32 = 3;

pub struct PixelArray {
    frame_width: u32,
    frame_crops: Vec<CropSetting>,
    buffer_capacity: usize,
}

impl PixelArray {
    pub fn new(frame_width: u32, frame_crops: Vec<CropSetting>) -> Self {
        let buffer_capacity = compute_output_buffer_size(&frame_crops);

        PixelArray {
            frame_width,
            frame_crops,
            buffer_capacity
        }
    }
}

impl FrameColorExtractionAlgorithm for PixelArray {
    fn process_frame(&mut self, frame_number: u64, pixels: &[u8]) -> Result<String> {
        let mut output = String::with_capacity(self.buffer_capacity);
        
        for crop in self.frame_crops.iter() {
            let resize = crop.resize_percentage;
            let pos_x = crop.pos_x_percentage;
            let pos_y = crop.pos_y_percentage;
            let crop_level = crop.crop_level as u8;

            write!(&mut output, "{frame_number} {resize} {pos_x} {pos_y} {crop_level}")?;

            for tile in crop.tiles.iter() {
                let mut total_red: u32 = 0;
                let mut total_green: u32 = 0;
                let mut total_blue: u32 = 0;

                for y in tile.start_y..tile.end_y {
                    let row_start = ((y * self.frame_width + tile.start_x) * BYTES_PER_PIXEL) as usize;
                    let row_end = ((y * self.frame_width + tile.end_x) * BYTES_PER_PIXEL) as usize;

                    for pixel in pixels[row_start..row_end].chunks_exact(3) {
                        total_red += pixel[0] as u32;
                        total_green += pixel[1] as u32;
                        total_blue += pixel[2] as u32;
                    }
                }

                let average_red = f64::round(total_red as f64 / tile.total_pixels as f64) as u8;
                let average_green = f64::round(total_green as f64 / tile.total_pixels as f64) as u8;
                let average_blue = f64::round(total_blue as f64 / tile.total_pixels as f64) as u8;
                
                write!(&mut output, " {average_red},{average_green},{average_blue}")?;
            }

            writeln!(&mut output)?;
        }

        Ok(output)
    }
}


#[cfg(test)]
mod tests {
    use super::PixelArray;
    use super::super::test_support::assert_correct_color_averages;

    #[test]
    fn correct_color_averages() {
        assert_correct_color_averages(|width, _height, crops| {
            PixelArray::new(width, crops)
        });
    }
}