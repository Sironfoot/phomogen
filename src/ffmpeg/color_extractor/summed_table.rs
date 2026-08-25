// see https://en.wikipedia.org/wiki/Summed-area_table

use crate::ffmpeg::{
    color_extractor::FrameColorExtractionAlgorithm,
    crops::CropSetting
};

use super::compute_output_buffer_size;

use std::fmt::Write;
use anyhow::Result;

pub struct SummedAreaTable {
    sums: Vec<[u32; 3]>,
    stride: u32,
    frame_width: u32,
    frame_height: u32,

    buffer_capacity: usize,
    frame_crops: Vec<CropSetting>,
}

impl SummedAreaTable {
    pub fn new(frame_width: u32, frame_height: u32, frame_crops: Vec<CropSetting>) -> Self {
        let stride = frame_width + 1;
        let sums = vec![[0u32; 3]; ((frame_width + 1) * (frame_height + 1)) as usize];

        let buffer_capacity = compute_output_buffer_size(&frame_crops);

        Self {
            sums,
            stride,
            frame_width,
            frame_height,
            buffer_capacity,
            frame_crops,
        }
    }

    fn init(&mut self, pixels: &[u8]) {
        for y in 0..self.frame_height {
            let mut row_red: u32 = 0;
            let mut row_green: u32 = 0;
            let mut row_blue: u32 = 0;

            let src_row = y * self.frame_width * 3;
            let dst_row = (y + 1) * self.stride;
            let prev_row = y * self.stride;

            for x in 0..self.frame_width {
                let src = (src_row + x * 3) as usize;

                row_red += pixels[src] as u32;
                row_green += pixels[src + 1] as u32;
                row_blue += pixels[src + 2] as u32;

                let dst = (dst_row + x + 1) as usize;
                let above = (prev_row + x + 1) as usize;

                self.sums[dst][0] = self.sums[above][0] + row_red;
                self.sums[dst][1] = self.sums[above][1] + row_green;
                self.sums[dst][2] = self.sums[above][2] + row_blue;
            }
        }
    }

    #[inline]
    fn sum_rect(&self, x1: u32, y1: u32, x2: u32, y2: u32) -> [u32; 3] {
        let a = (y1 * self.stride + x1) as usize;
        let b = (y1 * self.stride + x2) as usize;
        let c = (y2 * self.stride + x1) as usize;
        let d = (y2 * self.stride + x2) as usize;

        [
            self.sums[d][0] + self.sums[a][0] - self.sums[b][0] - self.sums[c][0],
            self.sums[d][1] + self.sums[a][1] - self.sums[b][1] - self.sums[c][1],
            self.sums[d][2] + self.sums[a][2] - self.sums[b][2] - self.sums[c][2],
        ]
    }

    #[inline]
    fn average_rect(&self, x1: u32, y1: u32, x2: u32, y2: u32) -> [u8; 3] {
        let sum = self.sum_rect(x1, y1, x2, y2);
        let count = ((x2 - x1) * (y2 - y1)) as u32;

        [
            ((sum[0] + count / 2) / count) as u8,
            ((sum[1] + count / 2) / count) as u8,
            ((sum[2] + count / 2) / count) as u8,
        ]
    }
}

impl FrameColorExtractionAlgorithm for SummedAreaTable {
    fn process_frame(&mut self, frame_number: u64, pixels: &[u8]) -> Result<String> {
        self.init(pixels);

        let mut output = String::with_capacity(self.buffer_capacity);

        for crop in self.frame_crops.iter() {
            let resize = crop.resize_percentage;
            let pos_x = crop.pos_x_percentage;
            let pos_y = crop.pos_y_percentage;

            write!(&mut output, "{frame_number} {resize} {pos_x} {pos_y}")?;

            for tile in crop.tiles.iter() {
                let [average_red, average_green, average_blue] = self
                    .average_rect(tile.start_x, tile.start_y, tile.end_x, tile.end_y);
                    
                write!(&mut output, " {average_red},{average_green},{average_blue}")?;
            }

            writeln!(&mut output)?;
        }

        Ok(output)
    }
}


#[cfg(test)]
mod tests {
    use super::SummedAreaTable;
    use super::super::test_support::assert_correct_color_averages;

    #[test]
    fn correct_color_averages() {
        assert_correct_color_averages(|width, height, crops| {
            SummedAreaTable::new(width, height, crops)
        });
    }
}