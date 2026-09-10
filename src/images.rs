use std::path::PathBuf;

use anyhow::Result;
use image::{DynamicImage, GenericImage, GenericImageView, imageops};

use crate::app::frame_data::Color;

const PREVIEW_IMAGE_MAX_SIZE: u32 = 320;
const PROGRESS_IMAGE_MAX_SIZE: u32 = 640;

pub struct PreviewImage {
    image: DynamicImage,
    progress_image: Option<DynamicImage>,

    mosaic_tiles_x: u32,
    mosaic_tiles_y: u32,
}

impl PreviewImage {
    pub fn new(image_path: &PathBuf) -> Result<PreviewImage> {
        let image = image::open(image_path)?;
        Ok(Self::from_image(image))
    }

    fn from_image(image: DynamicImage) -> PreviewImage {
        let (width, height) = image.dimensions();
        let (resize_width, resize_height) = resize_dimensions(width, height, PREVIEW_IMAGE_MAX_SIZE);

        let image = image.resize(resize_width, resize_height, imageops::FilterType::Lanczos3);

        PreviewImage {
            image,
            progress_image: None,
            mosaic_tiles_x: 0,
            mosaic_tiles_y: 0,
        }
    }

    pub fn generate_progress_image(&mut self, mosaic_tiles_x: u32, mosaic_tiles_y: u32) {
        self.mosaic_tiles_x = mosaic_tiles_x;
        self.mosaic_tiles_y = mosaic_tiles_y;

        let (width, height) = self.image.dimensions();
        let (resize_width, resize_height) = resize_dimensions(width, height, PROGRESS_IMAGE_MAX_SIZE);

        let progress_image = self.image.resize(resize_width, resize_height, imageops::FilterType::Lanczos3);
        self.progress_image = Some(progress_image);
    }

    pub fn add_progress_tile(&mut self, tile_x: u32, tile_y: u32) {
        if let Some(progress_image) = self.progress_image.as_mut() {
            let (width, height) = progress_image.dimensions();

            let tile_width = width as f64 / self.mosaic_tiles_x as f64;
            let tile_height = height as f64 / self.mosaic_tiles_y as f64;

            let left = tile_width * tile_x as f64;
            let right = left + tile_width;
            let top = tile_height * tile_y as f64;
            let bottom = top + tile_height;

            let left = left.round() as u32;
            let right = (right.round() as u32).min(width);
            let top = top.round() as u32;
            let bottom = (bottom.round() as u32).min(height);

            for y in top..bottom {
                for x in left..right {
                    let color = Color::from_rgba(progress_image.get_pixel(x, y))
                        .darken(50);

                    progress_image.put_pixel(x, y, color.to_rgba_color());
                }
            }
        }
    }

    pub fn image(&self) -> &DynamicImage {
        &self.image
    }

    pub fn progress_image(&self) -> Option<&DynamicImage> {
        match &self.progress_image {
            Some(image) => Some(image),
            None => None,
        }
    }
}

fn resize_dimensions(width: u32, height: u32, max_dimension_lengtth: u32) -> (u32, u32) {
    let is_landscape = width > height;
    let ratio = width as f64 / height as f64;

    match is_landscape {
        true => {
            let width = max_dimension_lengtth;
            let height = f64::round(width as f64 / ratio) as u32;
            (width, height)
        },
        false => {
            let height = max_dimension_lengtth;
            let width = f64::round(height as f64 * ratio) as u32;
            (width, height)
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use image::{DynamicImage, Rgba, RgbaImage};

    use super::*;

    #[test]
    fn test_resize_dimensions() {
        let tests_960: HashMap<(u32, u32), (u32, u32)> = HashMap::from([
            ((3840, 2160), (960, 540)),
            ((1920, 1080), (960, 540)),
            ((2160, 3840), (540, 960)),
            ((1080, 1920), (540, 960)),
        ]);

        let tests_320: HashMap<(u32, u32), (u32, u32)> = HashMap::from([
            ((3840, 2160), (320, 180)),
            ((1920, 1080), (320, 180)),
            ((2160, 3840), (180, 320)),
            ((1080, 1920), (180, 320)),
        ]);

        for ((width, height), (expected_width, expected_height)) in tests_960 {
            let (actual_width, actual_height) = resize_dimensions(width, height, 960);
            assert_eq!(expected_width, actual_width, "wrong width on 960px resize");
            assert_eq!(expected_height, actual_height, "wrong height on 960px resize");
        }

        for ((width, height), (expected_width, expected_height)) in tests_320 {
            let (actual_width, actual_height) = resize_dimensions(width, height, 320);
            assert_eq!(expected_width, actual_width, "wrong width on 320px resize");
            assert_eq!(expected_height, actual_height, "wrong height on 320px resize");
        }
    }

    #[test]
    fn test_progress_image() {
        let white_pixel = Rgba([255, 255, 255, 255]);
        let canvas = RgbaImage::from_pixel(320, 180, white_pixel);
        let image = DynamicImage::ImageRgba8(canvas);

        // check progress image is generated
        let mut preview_image = PreviewImage::from_image(image);
        preview_image.generate_progress_image(2, 2);
        assert!(preview_image.progress_image().is_some(), "progress image was not generated");

        // check pixels NOT changes
        let progress_image = preview_image.progress_image().unwrap();
        let (width, height) = progress_image.dimensions();

        for y in 0..(height / 2) {
            for x in 0..(width / 2) {
                let pixel = progress_image.get_pixel(x, y);
                assert_eq!(pixel, white_pixel, "pixels in top left corner should NOT have changed");
            }
        }

        // check pixels changed
        preview_image.add_progress_tile(0, 0);

        let progress_image = preview_image.progress_image().unwrap();
        let (width, height) = progress_image.dimensions();

        for y in 0..(height / 2) {
            for x in 0..(width / 2) {
                let pixel = progress_image.get_pixel(x, y);
                assert_ne!(pixel, white_pixel, "pixels in top left corner should have changed");
            }
        }

        // check pixels elsewhere in image have NOT changed
        for y in ((height / 2) + 1)..height { // plus 1 to factor in anti-aliasing/filter resize methods
            for x in ((width / 2) + 1)..width {
                let pixel = progress_image.get_pixel(x, y);
                assert_eq!(pixel, white_pixel, "pixels in bottom right corner should NOT have changed");
            }
        }
    }
}