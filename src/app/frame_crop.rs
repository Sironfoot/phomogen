use crate::{ffmpeg::crops::{CropLevel, CropSetting}, frame_matcher::{Color, OklabColor}};

use anyhow::Result;

#[derive(Debug)]
pub struct FrameCrop {
    pub resize_percentage: f32,
    pub crop_level: CropLevel,

    pub colors: Vec<Color>,
    pub oklab_colors: Option<Vec<OklabColor>>,
}

impl FrameCrop {
    pub fn init(resize_percentage: f32, crop_level: CropLevel) -> FrameCrop {
        FrameCrop {
            resize_percentage,
            crop_level,
            colors: vec![],
            oklab_colors: None,
        }
    }

    fn calculate_sub_grid_values(&self, start_x: u8, start_y: u8, crop: &CropSetting, total_colors: usize) -> Result<(usize, usize, usize, usize, usize)> {
        let full_width = crop.full_color_tiles_x() as usize;
        let full_height = crop.full_color_tiles_y() as usize;

        let width = crop.color_tiles_x() as usize;
        let height = crop.color_tiles_y() as usize;

        let start_x = start_x as usize;
        let end_x = start_x + width;

        let start_y = start_y as usize;
        let end_y = start_y + height;

        anyhow::ensure!(
            end_x <= full_width && end_y <= full_height,
            "Sub-grid ({start_x}, {start_y})..({end_x}, {end_y}) exceeds grid bounds ({full_width}, {full_height})"
        );

        let last_end = ((end_y - 1) * full_width) + end_x;
        anyhow::ensure!(
            last_end <= total_colors,
            "Sub-grid requires {last_end} colors, but only {total_colors} are available"
        );

        Ok((full_width, width, start_x, start_y, end_y))
    }

    fn extract_sub_grid_rows(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &[Color]> + Clone + '_> {
        let total_colors = self.colors.len();
        let (full_width, width, start_x, start_y, end_y) =
            self.calculate_sub_grid_values(start_x, start_y, crop, total_colors)?;

        Ok((start_y..end_y).map(move |y| {
            let start = (y * full_width) + start_x;
            &self.colors[start..start + width]
        }))
    }

    pub fn extract_sub_grid(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &Color> + '_> {
        Ok(self.extract_sub_grid_rows(start_x, start_y, crop)?.flat_map(|row| row.iter()))
    }

    pub fn extract_sub_grid_flipped(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &Color> + '_> {
        Ok(self.extract_sub_grid_rows(start_x, start_y, crop)?.flat_map(|row| row.iter().rev()))
    }

    fn extract_sub_grid_rows_oklab(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &[OklabColor]> + Clone + '_> {
        let oklab_colors = match &self.oklab_colors {
            Some(colors) => colors,
            None => {
                anyhow::bail!("OkLab colors have not been initialised")
            },
        };
        
        let total_colors = oklab_colors.len();
        let (full_width, width, start_x, start_y, end_y) =
            self.calculate_sub_grid_values(start_x, start_y, crop, total_colors)?;

        Ok((start_y..end_y).map(move |y| {
            let start = (y * full_width) + start_x;
            &oklab_colors[start..start + width]
        }))
    }

    pub fn extract_sub_grid_oklab(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &OklabColor> + '_> {
        Ok(self.extract_sub_grid_rows_oklab(start_x, start_y, crop)?.flat_map(|row| row.iter()))
    }

    pub fn extract_sub_grid_flipped_oklab(&self, start_x: u8, start_y: u8, crop: &CropSetting) -> Result<impl Iterator<Item = &OklabColor> + '_> {
        Ok(self.extract_sub_grid_rows_oklab(start_x, start_y, crop)?.flat_map(|row| row.iter().rev()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_frame_crop(resize: f32) -> (FrameCrop, CropSetting) {
        let crops = CropSetting::all_crops(1920, 1080, 8, 8).unwrap();

        let crop = crops.iter()
            .find(|c| c.resize_percentage() as f32 == resize).unwrap();
        let total_tiles = crop.full_color_tiles_x() * crop.full_color_tiles_y();

        let mut frame_crop = FrameCrop::init(resize, crop.crop_level);
        frame_crop.oklab_colors = Some(Vec::with_capacity(total_tiles as usize));

        for i in 0..total_tiles {
            frame_crop.colors.push(Color { r: i as u8, g: i as u8, b: i as u8 });
            frame_crop.oklab_colors.as_mut().unwrap().push(OklabColor { l: i as f32, a: i as f32, b: i as f32 });
        }

        (frame_crop, crop.clone())
    }

    #[test]
    fn extract_sub_grid_50() {
        let (frame_crop, crop) = get_frame_crop(50.0);

        // top left
        let top_left_colors: Vec<_> = frame_crop.extract_sub_grid(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_colors[0].r, 0, "top_left grid: color at top/left corner should be 0");
        assert_eq!(top_left_colors[7].r, 7, "top_left grid: color at top/right corner should be 7");
        assert_eq!(top_left_colors[56].r, 112, "top_left grid: color at bottom/left corner should be 112");
        assert_eq!(top_left_colors[63].r, 119, "top_left grid: color at bottom/right corner should be 119");

        let top_left_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_oklab[0].l, 0.0, "top_left (Oklab) grid: color at top/left corner should be 0");
        assert_eq!(top_left_oklab[7].l, 7.0, "top_left (Oklab) grid: color at top/right corner should be 7");
        assert_eq!(top_left_oklab[56].l, 112.0, "top_left (Oklab) grid: color at bottom/left corner should be 112");
        assert_eq!(top_left_oklab[63].l, 119.0, "top_left (Oklab)grid: color at bottom/right corner should be 119");

        // top right
        let top_right_colors: Vec<_> = frame_crop.extract_sub_grid(8, 0, &crop).unwrap().collect();
        assert_eq!(top_right_colors[0].r, 8, "top_right grid: color at top/left corner should be 8");
        assert_eq!(top_right_colors[7].r, 15, "top_right grid: color at top/right corner should be 15");
        assert_eq!(top_right_colors[56].r, 120, "top_right grid: color at bottom/left corner should be 120");
        assert_eq!(top_right_colors[63].r, 127, "top_right grid: color at bottom/right corner should be 127");

        let top_right_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(8, 0, &crop).unwrap().collect();
        assert_eq!(top_right_oklab[0].l, 8.0, "top_right (Oklab) grid: color at top/left corner should be 8");
        assert_eq!(top_right_oklab[7].l, 15.0, "top_right (Oklab) grid: color at top/right corner should be 15");
        assert_eq!(top_right_oklab[56].l, 120.0, "top_right (Oklab) grid: color at bottom/left corner should be 120");
        assert_eq!(top_right_oklab[63].l, 127.0, "top_right (Oklab) grid: color at bottom/right corner should be 127");

        // bottom left
        let bottom_left_colors: Vec<_> = frame_crop.extract_sub_grid(0, 8, &crop).unwrap().collect();
        assert_eq!(bottom_left_colors[0].r, 128, "bottom_left grid: color at top/left corner should be 128");
        assert_eq!(bottom_left_colors[7].r, 135, "bottom_left grid: color at top/right corner should be 135");
        assert_eq!(bottom_left_colors[56].r, 240, "bottom_left grid: color at bottom/left corner should be 240");
        assert_eq!(bottom_left_colors[63].r, 247, "bottom_left grid: color at bottom/right corner should be 247");

        let bottom_left_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(0, 8, &crop).unwrap().collect();
        assert_eq!(bottom_left_oklab[0].l, 128.0, "bottom_left (Oklab) grid: color at top/left corner should be 128");
        assert_eq!(bottom_left_oklab[7].l, 135.0, "bottom_left (Oklab) grid: color at top/right corner should be 135");
        assert_eq!(bottom_left_oklab[56].l, 240.0, "bottom_left (Oklab) grid: color at bottom/left corner should be 240");
        assert_eq!(bottom_left_oklab[63].l, 247.0, "bottom_left (Oklab) grid: color at bottom/right corner should be 247");

        // bottom right
        let bottom_right_colors: Vec<_> = frame_crop.extract_sub_grid(8, 8, &crop).unwrap().collect();
        assert_eq!(bottom_right_colors[0].r, 136, "bottom_right grid: color at top/left corner should be 136");
        assert_eq!(bottom_right_colors[7].r, 143, "bottom_right grid: color at top/right corner should be 143");
        assert_eq!(bottom_right_colors[56].r, 248, "bottom_right grid: color at bottom/left corner should be 248");
        assert_eq!(bottom_right_colors[63].r, 255, "bottom_right grid: color at bottom/right corner should be 255");

        let bottom_right_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(8, 8, &crop).unwrap().collect();
        assert_eq!(bottom_right_oklab[0].l, 136.0, "bottom_right (Oklab) grid: color at top/left corner should be 136");
        assert_eq!(bottom_right_oklab[7].l, 143.0, "bottom_right (Oklab) grid: color at top/right corner should be 143");
        assert_eq!(bottom_right_oklab[56].l, 248.0, "bottom_right (Oklab) grid: color at bottom/left corner should be 248");
        assert_eq!(bottom_right_oklab[63].l, 255.0, "bottom_right (Oklab) grid: color at bottom/right corner should be 255");
    }

    #[test]
    fn extract_sub_grid_flipped_50() {
        let (frame_crop, crop) = get_frame_crop(50.0);

        // top left
        let top_left_colors: Vec<_> = frame_crop.extract_sub_grid_flipped(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_colors[0].r, 7, "top_left grid: color at top/left corner should be 7");
        assert_eq!(top_left_colors[7].r, 0, "top_left grid: color at top/right corner should be 0");
        assert_eq!(top_left_colors[56].r, 119, "top_left grid: color at bottom/left corner should be 119");
        assert_eq!(top_left_colors[63].r, 112, "top_left grid: color at bottom/right corner should be 112");

        let top_left_oklab: Vec<_> = frame_crop.extract_sub_grid_flipped_oklab(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_oklab[0].l, 7.0, "top_left (Oklab) grid: color at top/left corner should be 7");
        assert_eq!(top_left_oklab[7].l, 0.0, "top_left (Oklab) grid: color at top/right corner should be 0");
        assert_eq!(top_left_oklab[56].l, 119.0, "top_left (Oklab) grid: color at bottom/left corner should be 119");
        assert_eq!(top_left_oklab[63].l, 112.0, "top_left (Oklab) grid: color at bottom/right corner should be 112");

        // top right
        let top_right_colors: Vec<_> = frame_crop.extract_sub_grid_flipped(8, 0, &crop).unwrap().collect();
        assert_eq!(top_right_colors[0].r, 15, "top_right grid: color at top/left corner should be 15");
        assert_eq!(top_right_colors[7].r, 8, "top_right grid: color at top/right corner should be 8");
        assert_eq!(top_right_colors[56].r, 127, "top_right grid: color at bottom/left corner should be 127");
        assert_eq!(top_right_colors[63].r, 120, "top_right grid: color at bottom/right corner should be 120");

        let top_right_oklab: Vec<_> = frame_crop.extract_sub_grid_flipped_oklab(8, 0, &crop).unwrap().collect();
        assert_eq!(top_right_oklab[0].l, 15.0, "top_right (Oklab) grid: color at top/left corner should be 15");
        assert_eq!(top_right_oklab[7].l, 8.0, "top_right (Oklab) grid: color at top/right corner should be 8");
        assert_eq!(top_right_oklab[56].l, 127.0, "top_right (Oklab) grid: color at bottom/left corner should be 127");
        assert_eq!(top_right_oklab[63].l, 120.0, "top_right (Oklab) grid: color at bottom/right corner should be 120");

        // bottom left
        let bottom_left_colors: Vec<_> = frame_crop.extract_sub_grid_flipped(0, 8, &crop).unwrap().collect();
        assert_eq!(bottom_left_colors[0].r, 135, "bottom_left grid: color at top/left corner should be 135");
        assert_eq!(bottom_left_colors[7].r, 128, "bottom_left grid: color at top/right corner should be 128");
        assert_eq!(bottom_left_colors[56].r, 247, "bottom_left grid: color at bottom/left corner should be 247");
        assert_eq!(bottom_left_colors[63].r, 240, "bottom_left grid: color at bottom/right corner should be 240");

        let bottom_left_oklab: Vec<_> = frame_crop.extract_sub_grid_flipped_oklab(0, 8, &crop).unwrap().collect();
        assert_eq!(bottom_left_oklab[0].l, 135.0, "bottom_left (Oklab) grid: color at top/left corner should be 135");
        assert_eq!(bottom_left_oklab[7].l, 128.0, "bottom_left (Oklab) grid: color at top/right corner should be 128");
        assert_eq!(bottom_left_oklab[56].l, 247.0, "bottom_left (Oklab) grid: color at bottom/left corner should be 247");
        assert_eq!(bottom_left_oklab[63].l, 240.0, "bottom_left (Oklab) grid: color at bottom/right corner should be 240");

        // bottom right
        let bottom_right_colors: Vec<_> = frame_crop.extract_sub_grid_flipped(8, 8, &crop).unwrap().collect();
        assert_eq!(bottom_right_colors[0].r, 143, "bottom_right grid: color at top/left corner should be 143");
        assert_eq!(bottom_right_colors[7].r, 136, "bottom_right grid: color at top/right corner should be 136");
        assert_eq!(bottom_right_colors[56].r, 255, "bottom_right grid: color at bottom/left corner should be 255");
        assert_eq!(bottom_right_colors[63].r, 248, "bottom_right grid: color at bottom/right corner should be 248");

        let bottom_right_oklab: Vec<_> = frame_crop.extract_sub_grid_flipped_oklab(8, 8, &crop).unwrap().collect();
        assert_eq!(bottom_right_oklab[0].l, 143.0, "bottom_right (Oklab) grid: color at top/left corner should be 143");
        assert_eq!(bottom_right_oklab[7].l, 136.0, "bottom_right (Oklab) grid: color at top/right corner should be 136");
        assert_eq!(bottom_right_oklab[56].l, 255.0, "bottom_right (Oklab) grid: color at bottom/left corner should be 255");
        assert_eq!(bottom_right_oklab[63].l, 248.0, "bottom_right (Oklab) grid: color at bottom/right corner should be 248");
    }

    #[test]
    fn extract_sub_grid_66() {
        let (frame_crop, crop) = get_frame_crop(66.666);

        // top left
        let top_left_colors: Vec<_> = frame_crop.extract_sub_grid(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_colors[0].r, 0, "top_left grid: color at top/left corner should be 0");
        assert_eq!(top_left_colors[7].r, 7, "top_left grid: color at top/right corner should be 7");
        assert_eq!(top_left_colors[56].r, 84, "top_left grid: color at bottom/left corner should be 84");
        assert_eq!(top_left_colors[63].r, 91, "top_left grid: color at bottom/right corner should be 91");

        let top_left_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(0, 0, &crop).unwrap().collect();
        assert_eq!(top_left_oklab[0].l, 0.0, "top_left (Oklab) grid: color at top/left corner should be 0");
        assert_eq!(top_left_oklab[7].l, 7.0, "top_left (Oklab) grid: color at top/right corner should be 7");
        assert_eq!(top_left_oklab[56].l, 84.0, "top_left (Oklab) grid: color at bottom/left corner should be 84");
        assert_eq!(top_left_oklab[63].l, 91.0, "top_left (Oklab) grid: color at bottom/right corner should be 91");

        // top right
        let top_right_colors: Vec<_> = frame_crop.extract_sub_grid(4, 0, &crop).unwrap().collect();
        assert_eq!(top_right_colors[0].r, 4, "top_right grid: color at top/left corner should be 4");
        assert_eq!(top_right_colors[7].r, 11, "top_right grid: color at top/right corner should be 11");
        assert_eq!(top_right_colors[56].r, 88, "top_right grid: color at bottom/left corner should be 88");
        assert_eq!(top_right_colors[63].r, 95, "top_right grid: color at bottom/right corner should be 95");

        let top_right_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(4, 0, &crop).unwrap().collect();
        assert_eq!(top_right_oklab[0].l, 4.0, "top_right (Oklab) grid: color at top/left corner should be 4");
        assert_eq!(top_right_oklab[7].l, 11.0, "top_right (Oklab) grid: color at top/right corner should be 11");
        assert_eq!(top_right_oklab[56].l, 88.0, "top_right (Oklab) grid: color at bottom/left corner should be 88");
        assert_eq!(top_right_oklab[63].l, 95.0, "top_right (Oklab) grid: color at bottom/right corner should be 95");

        // bottom left
        let bottom_left_colors: Vec<_> = frame_crop.extract_sub_grid(0, 4, &crop).unwrap().collect();
        assert_eq!(bottom_left_colors[0].r, 48, "bottom_left grid: color at top/left corner should be 48");
        assert_eq!(bottom_left_colors[7].r, 55, "bottom_left grid: color at top/right corner should be 55");
        assert_eq!(bottom_left_colors[56].r, 132, "bottom_left grid: color at bottom/left corner should be 132");
        assert_eq!(bottom_left_colors[63].r, 139, "bottom_left grid: color at bottom/right corner should be 139");

        let bottom_left_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(0, 4, &crop).unwrap().collect();
        assert_eq!(bottom_left_oklab[0].l, 48.0, "bottom_left (Oklab) grid: color at top/left corner should be 48");
        assert_eq!(bottom_left_oklab[7].l, 55.0, "bottom_left (Oklab) grid: color at top/right corner should be 55");
        assert_eq!(bottom_left_oklab[56].l, 132.0, "bottom_left (Oklab) grid: color at bottom/left corner should be 132");
        assert_eq!(bottom_left_oklab[63].l, 139.0, "bottom_left (Oklab)grid: color at bottom/right corner should be 139");

        // bottom right
        let bottom_right_colors: Vec<_> = frame_crop.extract_sub_grid(4, 4, &crop).unwrap().collect();
        assert_eq!(bottom_right_colors[0].r, 52, "bottom_right grid: color at top/left corner should be 52");
        assert_eq!(bottom_right_colors[7].r, 59, "bottom_right grid: color at top/right corner should be 59");
        assert_eq!(bottom_right_colors[56].r, 136, "bottom_right grid: color at bottom/left corner should be 136");
        assert_eq!(bottom_right_colors[63].r, 143, "bottom_right grid: color at bottom/right corner should be 143");

        let bottom_right_oklab: Vec<_> = frame_crop.extract_sub_grid_oklab(4, 4, &crop).unwrap().collect();
        assert_eq!(bottom_right_oklab[0].l, 52.0, "bottom_right (Oklab) grid: color at top/left corner should be 52");
        assert_eq!(bottom_right_oklab[7].l, 59.0, "bottom_right (Oklab) grid: color at top/right corner should be 59");
        assert_eq!(bottom_right_oklab[56].l, 136.0, "bottom_right (Oklab) grid: color at bottom/left corner should be 136");
        assert_eq!(bottom_right_oklab[63].l, 143.0, "bottom_right (Oklab) grid: color at bottom/right corner should be 143");
    }
}