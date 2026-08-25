use image::{Rgb, RgbImage};

use super::{FrameColorExtractionAlgorithm, PixelArray, SummedAreaTable};
use crate::ffmpeg::crops::CropSetting;

fn assert_correct_color_averages<A>(create_algorithm: impl FnOnce(u32, u32, Vec<CropSetting>) -> A)
where
    A: FrameColorExtractionAlgorithm,
{
    const FRAME_WIDTH: u32 = 1_920;
    const FRAME_HEIGHT: u32 = 1_080;
    const GRID_SIZE: u32 = 4;

    let colors = [
        Rgb([255, 0, 0]),
        Rgb([0, 255, 0]),
        Rgb([0, 0, 255]),
        Rgb([255, 255, 0]),
        Rgb([255, 0, 255]),
        Rgb([0, 255, 255]),
        Rgb([255, 128, 0]),
        Rgb([128, 0, 255]),
        Rgb([0, 128, 255]),
        Rgb([128, 255, 0]),
        Rgb([255, 0, 128]),
        Rgb([0, 255, 128]),
        Rgb([64, 64, 64]),
        Rgb([128, 128, 128]),
        Rgb([192, 192, 192]),
        Rgb([255, 255, 255]),
    ];
    let frame_index: u64 = 123;

    let cell_width = FRAME_WIDTH / GRID_SIZE;
    let cell_height = FRAME_HEIGHT / GRID_SIZE;

    let image = RgbImage::from_fn(FRAME_WIDTH, FRAME_HEIGHT, |x, y| {
        let color_index = (y / cell_height * GRID_SIZE + x / cell_width) as usize;
        colors[color_index]
    });
    let pixels: &[u8] = image.as_raw();

    let frame_crops = CropSetting::all_crops(
        FRAME_WIDTH,
        FRAME_HEIGHT,
        GRID_SIZE,
        GRID_SIZE).unwrap();

    let total_frame_crops = frame_crops.len();

    let mut color_extractor = create_algorithm(FRAME_WIDTH, FRAME_HEIGHT, frame_crops);
    let output = color_extractor.process_frame(frame_index, pixels);

    // output shouldn't result in error
    if let Err(err) = output {
        panic!("process_frame returned an error: {}", err);
    }

    let lines = output.unwrap()
        .lines()
        .map(String::from)
        .collect::<Vec<String>>();

    // check number of lines match number of crops
    assert_eq!(lines.len(), total_frame_crops, "number of lines should match number of crops");

    let frame_crops = CropSetting::all_crops(
        FRAME_WIDTH,
        FRAME_HEIGHT,
        GRID_SIZE,
        GRID_SIZE).unwrap();

    // [Frame Index] [Crop Size] [PosX] [PosY] [R,G,B] [R,G,B] [R,G,B] [R,G,B]
    for (i, crop) in frame_crops.iter().enumerate() {
        let mut expected_line = format!("{frame_index} {} {} {}", crop.resize_percentage, crop.pos_x_percentage, crop.pos_y_percentage);
        let line = &lines[i];

        // check full frame which should exactly match RGB array above
        if i == 0 {
            for color in &colors {
                let [r, g, b] = color.0;
                expected_line.push_str(&format!(" {r},{g},{b}"));
            }

            assert_eq!(*line, expected_line, "full frame colors do not match");
        }
        else {
            assert!(line.starts_with(&expected_line),
                "line {i} does not start with `{expected_line}`. Full line: `{line}`");
        }
    }
}


#[test]
fn pixel_array_returns_correct_color_averages() {
    assert_correct_color_averages(|width, _height, crops| {
        PixelArray::new(width, crops)
    });
}

#[test]
fn summed_area_table_returns_correct_color_averages() {
    assert_correct_color_averages(|width, height, crops| {
        SummedAreaTable::new(width, height, crops)
    });
}