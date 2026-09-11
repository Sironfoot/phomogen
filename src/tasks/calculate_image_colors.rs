use std::{cmp, sync::mpsc::{self, Receiver}, thread};

use image::{GenericImageView, imageops};
use image::imageops::FilterType;

use crate::{app::{App, frame_data::Color}, color_matcher::ImageTile};

pub fn run(app: &App) -> Receiver<Vec<ImageTile>> {
    let (tx, rc) = mpsc::channel::<Vec<ImageTile>>();

    let image = app.images.iter().find(|i| i.is_chosen)
        .expect("No image is chosen");
    let image_path = image.full_path.clone();

    let color_tiles_x = app.color_tiles_x;
    let color_tiles_y = app.color_tiles_y;

    let mosaic_tiles_x = app.mosaic_tiles_x;
    let mosaic_tiles_y = app.mosaic_tiles_y;

    thread::spawn(move || {
        let total_tiles = mosaic_tiles_x * mosaic_tiles_y;
        let mut image_tiles: Vec<ImageTile> = Vec::with_capacity(total_tiles as usize);

        let image_file = image::open(image_path).unwrap();
        let image_data = image_file.to_rgb8();

        const LARGEST_DIMENSION: u32 = 10000;
        let (image_width, image_height) = image_file.dimensions();

        let is_landscape = image_width > image_height;
        let ratio = image_width as f64 / image_height as f64;

        let smallest_dimension = if is_landscape {
            (LARGEST_DIMENSION as f64 / ratio).round() as u32
        }
        else
        {
            (LARGEST_DIMENSION as f64 * ratio).round() as u32
        };

        let target_width: u32 = if is_landscape { LARGEST_DIMENSION } else { smallest_dimension };
        let target_height: u32 = if is_landscape { smallest_dimension } else { LARGEST_DIMENSION };

        let mosaic_tile_width = f64::round(target_width as f64 / mosaic_tiles_x as f64) as u32;
        let mosaic_tile_height = f64::round(target_height as f64 / mosaic_tiles_y as f64) as u32;

        let resize_width = mosaic_tile_width * mosaic_tiles_x;
        let resize_height = mosaic_tile_height * mosaic_tiles_y;

        let image_data = imageops::resize(
            &image_data, resize_width, resize_height, FilterType::CatmullRom);

        let color_tile_width = f64::round(mosaic_tile_width as f64 / color_tiles_x as f64) as u32;
        let color_tile_height = f64::round(mosaic_tile_height as f64 / color_tiles_y as f64) as u32;

        let total_sub_tile_pixels = color_tile_width * color_tile_height;

        for tile_y in 0..mosaic_tiles_y {
            for tile_x in 0..mosaic_tiles_x {
                let start_x = tile_x * mosaic_tile_width;
                let start_y = tile_y * mosaic_tile_height;

                let sub_image = image_data.view(start_x, start_y, mosaic_tile_width, mosaic_tile_height);

                let mut tile_data = ImageTile {
                    colors: vec![]
                };
                
                for sub_tile_y in 0..color_tiles_y {
                    for sub_tile_x in 0..color_tiles_x {
                        let start_x = sub_tile_x * color_tile_width;
                        let end_x = cmp::min(start_x + color_tile_width, mosaic_tile_width);

                        let start_y = sub_tile_y * color_tile_height;
                        let end_y = cmp::min(start_y + color_tile_height, mosaic_tile_height);

                        let mut total_red: u64 = 0;
                        let mut total_green: u64 = 0;
                        let mut total_blue: u64 = 0;

                        for pixel_y in start_y..end_y {
                            for pixel_x in start_x..end_x {
                                let pixel = sub_image.get_pixel(pixel_x, pixel_y);
                                let [red, green, blue] = pixel.0;

                                total_red += red as u64;
                                total_green += green as u64;
                                total_blue += blue as u64;
                            }
                        }

                        let average_red = f64::round(total_red as f64 / total_sub_tile_pixels as f64) as u64;
                        let average_green = f64::round(total_green as f64 / total_sub_tile_pixels as f64) as u64;
                        let average_blue = f64::round(total_blue as f64 / total_sub_tile_pixels as f64) as u64;

                        tile_data.colors.push(Color {
                            r: average_red as u8,
                            g: average_green as u8,
                            b: average_blue as u8,
                        });
                    }
                }

                image_tiles.push(tile_data);
            }
        }

        tx.send(image_tiles).unwrap();
    });

    rc
}