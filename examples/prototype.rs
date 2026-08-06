use std::collections::HashMap;
use std::{io, path::Path};
use std::fs::{File, exists};
use std::io::{BufRead, Write};

use image::imageops::FilterType;
use ndarray::Array3;
use video_rs::{decode::Decoder, Frame};
use image::{GenericImage, GenericImageView, RgbImage, imageops};

const GRIDS_X: u32 = 3;
const GRIDS_Y: u32 = 3;

const MOSAIC_TILES: u32 = 40;

const DATA_FILE: &str = "data.txt";
const VIDEO_FILE: &str = "sample.mp4";
const TEST_IMAGE: &str = "test-image.jpeg";

#[derive(Debug)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug)]
struct FrameData {
    blocks: Vec<Color>,
}

fn main() {
    let data_exists = exists(DATA_FILE).unwrap();

    if !data_exists {
        println!("Generating Database");
        generate_database();
    }
    
    let file = File::open(DATA_FILE).unwrap();
    let lines = io::BufReader::new(file).lines();

    let mut data: HashMap<u64, FrameData> = HashMap::new();

    let mut current_iframe_ndex: u64 = 0;
    let mut current_frame = FrameData { blocks: vec![] };

    for line in lines.map_while(Result::ok) {
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            eprintln!("Invalid data: {line}");
            break;
        }

        let index: u64 = parts[0].parse().unwrap();
        if index > current_iframe_ndex {
            data.insert(current_iframe_ndex, current_frame);

            current_iframe_ndex = index;
            current_frame = FrameData { blocks: vec![] };
        }

        //let coords = parts[1];

        let rgb: Vec<&str> = parts[2].split(',').collect();

        let r: u8 = rgb[0].parse().unwrap();
        let g: u8 = rgb[1].parse().unwrap();
        let b: u8 = rgb[2].parse().unwrap();

        current_frame.blocks.push(Color { r, g, b });
    }

    println!("Loaded Data. {} frames processed!", data.len());

    let image = image::open(TEST_IMAGE).unwrap().to_rgb8();
    let (width, height) = image.dimensions();

    let mut image_data: Vec<FrameData> = vec![];

    
    let tile_width = width / MOSAIC_TILES;
    let tile_height = height / MOSAIC_TILES;

    let sub_tile_width = tile_width / GRIDS_X;
    let sub_tile_height = tile_height / GRIDS_Y;

    let total_sub_tile_pixels = sub_tile_width * sub_tile_height;

    for tile_y in 0..MOSAIC_TILES {
        for tile_x in 0..MOSAIC_TILES {
            let start_x = tile_x * tile_width;
            let start_y = tile_y * tile_height;

            let sub_image = image.view(start_x, start_y, tile_width, tile_height);

            let mut frame_data = FrameData {
                blocks: vec![]
            };
            
            for sub_tile_y in 0..GRIDS_Y {
                for sub_tile_x in 0..GRIDS_X {
                    let start_x = sub_tile_x * sub_tile_width;
                    let end_x = start_x + sub_tile_width;

                    let start_y = sub_tile_y * sub_tile_height;
                    let end_y = start_y + sub_tile_height;

                    let mut total_red: u32 = 0;
                    let mut total_green: u32 = 0;
                    let mut total_blue: u32 = 0;

                    for pixel_y in start_y..end_y {
                        for pixel_x in start_x..end_x {
                            let pixel = sub_image.get_pixel(pixel_x, pixel_y);
                            let [red, green, blue] = pixel.0;

                            total_red += red as u32;
                            total_green += green as u32;
                            total_blue += blue as u32;
                        }
                    }

                    let average_red = total_red / total_sub_tile_pixels;
                    let average_green = total_green / total_sub_tile_pixels;
                    let average_blue = total_blue / total_sub_tile_pixels;

                    frame_data.blocks.push(Color {
                        r: average_red as u8,
                        g: average_green as u8,
                        b: average_blue as u8,
                    });
                }
            }

            image_data.push(frame_data);
        }
    }

    println!("Image color data processed");

    // UNCOMMENT TO TEST COLOR AVERAGES
    // let image = RgbImage::from_fn(width, height, |x, y| {
    //     let tile_x = x / tile_width;
    //     let tile_y = y / tile_height;

    //     let tile_index = (tile_y * MOSAIC_TILES + tile_x) as usize;
    //     let tile_data = &image_data[tile_index];

    //     let sub_tile_x = (x - (tile_width * tile_x)) / sub_tile_width;
    //     let sub_tile_y = (y - (tile_height * tile_y)) / sub_tile_height;

    //     let sub_tile_index = (sub_tile_y * GRIDS_Y + sub_tile_x) as usize;
    //     //println!("{sub_tile_x}x{sub_tile_y} = {sub_tile_index}");

    //     let color = &tile_data.blocks[sub_tile_index];

    //     Rgb([color.r, color.g, color.b])
    // });

    // image.save("test-colors.png").unwrap();

    println!("Searching nearest matches");

    let mut selected_frames : Vec<u64> = vec![];

    for image_y in 0..MOSAIC_TILES {
        for image_x in 0..MOSAIC_TILES {
            let tile_index = (image_y * MOSAIC_TILES + image_x) as usize;
            let image_tile_data = &image_data[tile_index];

            let frame_index = find_nearest_color(&data, image_tile_data);
            //println!("{image_x}x{image_y} = {}", frame_index);

            selected_frames.push(frame_index);
        }
    }

    println!("Generating Mosaic");
    generate_mosaic(selected_frames);

}

fn generate_mosaic(frame_indexes: Vec<u64>) {
    video_rs::init().unwrap();

    let mut decoder = Decoder::new(Path::new(VIDEO_FILE)).unwrap();

    let mut sort_indexes = frame_indexes.clone();
    sort_indexes.sort_unstable();
    sort_indexes.dedup();

    let last_required_frame = *sort_indexes.iter().max().unwrap();

    let mut results: HashMap<u64, Frame> = HashMap::with_capacity(sort_indexes.len());

    for frame_index in 0..=last_required_frame {
        print!("\rProcessing frame: {frame_index}");

        let (_, frame) = decoder.decode().unwrap();

        if !sort_indexes.contains(&frame_index) {
            continue;
        }

        // let pixels: Vec<u8> = frame.iter().copied().collect();
        // let image = RgbImage::from_raw(3840, 2160, pixels).unwrap();
        // image.save(format!("test/{frame_number}.jpg")).unwrap();

        results.insert(frame_index as u64, frame);
    }

    println!("");
    println!("Building canvas");

    let cell_width: u32 = 240;
    let cell_height: u32 = 135;

    let full_width = cell_width * MOSAIC_TILES;
    let full_height = cell_height * MOSAIC_TILES;

    let mut canvas = RgbImage::new(full_width, full_height);

    for (i, frame_index) in frame_indexes.iter().enumerate() {
        let frame = results.get(frame_index).unwrap();
        let pixels = frame.as_slice().unwrap().to_vec();
        let image = RgbImage::from_raw(1920, 1080, pixels).unwrap();

        let row = i as u32 / MOSAIC_TILES;
        let col = i as u32 % MOSAIC_TILES;

        let resized = imageops::resize(&image, cell_width, cell_height, FilterType::Triangle);

        canvas.copy_from(&resized, col * cell_width, row * cell_height).unwrap();
    }

    canvas.save("output.jpeg").unwrap();
}

fn find_nearest_color(frames: &HashMap<u64, FrameData>, to_match: &FrameData) -> u64 {
    let mut nearest_match: Option<u64> = None;
    let mut smallest_distance = std::u64::MAX;

    const RED_BIAS: u64 = 3;
    const GREEN_BIAS: u64 = 6;
    const BLUE_BIAS: u64 = 1;

    for (frame_index, frame) in frames {
        let mut distance: u64 = 0;

        for i in 0..to_match.blocks.len() {
            let image_color = &to_match.blocks[i];
            let frame_color = &frame.blocks[i];

            let red_dist = frame_color.r.abs_diff(image_color.r) as u64;
            let green_dist = frame_color.g.abs_diff(image_color.g) as u64;
            let blue_dist = frame_color.b.abs_diff(image_color.b) as u64;

            distance +=
                (RED_BIAS * red_dist * red_dist) +
                (GREEN_BIAS * green_dist * green_dist) +
                (BLUE_BIAS * blue_dist * blue_dist);
        }

        if distance < smallest_distance {
			nearest_match = Some(*frame_index);
			smallest_distance = distance;
		}
    }

    return nearest_match.unwrap();
}

fn generate_database() {
    video_rs::init()
        .expect("Failed to init video_rs");

    let mut data = File::create(DATA_FILE).unwrap();
    //writeln!(&mut data, "test.mp4").unwrap();

    let mut decoder = Decoder::new(Path::new(VIDEO_FILE))
        .expect("Failed to get decoder");

    let (width, height) = decoder.size();
    let frame_rate = decoder.frame_rate();
    println!("Video size is {}x{}", width, height);
    println!("Frame rate: {}fps", frame_rate);

    let time = decoder.duration().unwrap();
    let in_secs = time.as_secs();
    println!("Total video length is: {}s", in_secs);
    let total_frames = decoder.frames().unwrap();
    println!("Total frames are: {}", total_frames);

    for (frame_number, decoded) in decoder.decode_iter().enumerate() {
        if let Err(e) = decoded {
            eprintln!("{:?}", e);
            break;
        }

        let (_, frame): (_, Array3<u8>) = decoded.unwrap();
        process_frame(width, height, &frame, frame_number, &mut data);

        if frame_number % 123 == 0 {
            let percentage_complete = (frame_number as f64 / total_frames as f64) * 100.0;
            print!("\rProcessed frame: {frame_number} ({:.2}%)", percentage_complete);
            std::io::stdout().flush().unwrap();
        }
    }

    println!("");
    println!("Done")
}

fn process_frame(width: u32, height: u32, frame: &Array3<u8>, frame_number: usize, data: &mut File) {
    let grid_width = width / GRIDS_X;
    let grid_height = height / GRIDS_Y;

    let total_grid_pixels: u64 = (grid_width * grid_height) as u64;

    for grid_y in 0..GRIDS_Y {
        for grid_x in 0..GRIDS_X {
            let start_x = grid_width * grid_x;
            let end_x = start_x + grid_width;

            let start_y = grid_height * grid_y;
            let end_y = start_y + grid_height;

            let mut total_red: u64 = 0;
            let mut total_green: u64 = 0;
            let mut total_blue: u64 = 0; 

            for x in start_x..end_x {
                for y in start_y..end_y {
                    let red = frame[[y as usize, x as usize, 0]] as u64;
                    total_red += red;

                    let green = frame[[y as usize, x as usize, 1]] as u64;
                    total_green += green;

                    let blue = frame[[y as usize, x as usize, 2]] as u64;
                    total_blue += blue;
                }
            }

            let average_red = total_red / total_grid_pixels;
            let average_green = total_green / total_grid_pixels;
            let average_blue = total_blue / total_grid_pixels;

            writeln!(data, "{frame_number} {grid_x},{grid_y} {average_red},{average_green},{average_blue}").unwrap();
        }
    }
}