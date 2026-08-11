use std::error::Error;
use std::fs::{File, exists};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::{self, BufRead, Read, Write};
use std::time::Instant;

use image::{GenericImage, GenericImageView, ImageBuffer, Rgb, RgbImage, imageops};
use image::imageops::FilterType;

use num_format::{Locale, ToFormattedString};

struct VideoMetaData {
    width: u32,
    height: u32,
    frame_rate: f64,
    total_frames: u64,
}

#[derive(Debug)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug)]
struct FrameData {
    full_frame: Vec<Color>,
    crops: Vec<FrameCrop>,
}

#[derive(Debug)]
struct FrameCrop {
    resize_percentage: f64,
    pos_x_percentage: f64,
    pos_y_percentage: f64,

    colors: Vec<Color>,
}

impl FrameData {
    fn new(grid_x: u32, grid_y: u32) -> FrameData {
        let grid_tiles = (grid_x * grid_y) as usize;

        FrameData {
            full_frame: Vec::with_capacity(grid_tiles),
            crops: vec![],
        }
    }
}

struct CropSettings {
    resize_percentage: f64,
    pos_x_percentage: f64,
    pos_y_percentage: f64,

    crop_start_x: u32,
    crop_start_y: u32,

    cropped_grid_width: u32,
    cropped_grid_height: u32,
    cropped_total_grid_pixels: u64,
}

impl CropSettings {
    fn new(resize: f64, pos_x: f64, pos_y: f64, full_width: u32, full_height: u32, grids_x: u32, grids_y: u32) -> CropSettings {
        let cropped_width = f64::round((full_width as f64 / 100.0) * resize) as u32;
        let cropped_height = f64::round((full_height as f64 / 100.0) * resize) as u32;

        let crop_start_x = f64::round((full_width as f64 / 100.0) * pos_x) as u32;
        let crop_start_y = f64::round((full_height as f64 / 100.0) * pos_y) as u32;

        let cropped_grid_width = f64::round(cropped_width as f64 / grids_x as f64) as u32;
        let cropped_grid_height = f64::round(cropped_height as f64 / grids_y as f64) as u32;
        let cropped_total_grid_pixels = (cropped_grid_width * cropped_grid_height) as u64;

        CropSettings {
            resize_percentage: resize,
            pos_x_percentage: pos_x,
            pos_y_percentage: pos_y,

            crop_start_x: crop_start_x,
            crop_start_y: crop_start_y,

            cropped_grid_width: cropped_grid_width,
            cropped_grid_height: cropped_grid_height,
            cropped_total_grid_pixels: cropped_total_grid_pixels,
        }
    }

    fn all_crops(full_width: u32, full_height: u32, grids_x: u32, grids_y: u32) -> Vec<CropSettings> {
        let frame_crops: Vec<CropSettings> = vec![
            // 50% crops
            CropSettings::new(50.0, 0.0, 0.0, full_width, full_height, grids_x, grids_y),   // top left
            CropSettings::new(50.0, 25.0, 0.0, full_width, full_height, grids_x, grids_y),  // top
            CropSettings::new(50.0, 50.0, 0.0, full_width, full_height, grids_x, grids_y),  // top right
            CropSettings::new(50.0, 0.0, 25.0, full_width, full_height, grids_x, grids_y),  // left
            CropSettings::new(50.0, 25.0, 25.0, full_width, full_height, grids_x, grids_y), // center
            CropSettings::new(50.0, 50.0, 25.0, full_width, full_height, grids_x, grids_y), // right
            CropSettings::new(50.0, 0.0, 50.0, full_width, full_height, grids_x, grids_y),  // bottom left
            CropSettings::new(50.0, 25.0, 50.0, full_width, full_height, grids_x, grids_y), // bottom
            CropSettings::new(50.0, 50.0, 50.0, full_width, full_height, grids_x, grids_y), // bottom right

            // 50% inner crops
            CropSettings::new(50.0, 12.5, 12.5, full_width, full_height, grids_x, grids_y), // inner top left
            CropSettings::new(50.0, 37.5, 12.5, full_width, full_height, grids_x, grids_y), // inner top right
            CropSettings::new(50.0, 12.5, 37.5, full_width, full_height, grids_x, grids_y), // inner bottom left
            CropSettings::new(50.0, 37.5, 37.5, full_width, full_height, grids_x, grids_y), // inner bottom right

            // 66.666% crops
            CropSettings::new(66.666, 0.0, 0.0, full_width, full_height, grids_x, grids_y),       // top left
            CropSettings::new(66.666, 16.666, 0.0, full_width, full_height, grids_x, grids_y),    // top
            CropSettings::new(66.666, 33.333, 0.0, full_width, full_height, grids_x, grids_y),    // top right
            CropSettings::new(66.666, 0.0, 16.666, full_width, full_height, grids_x, grids_y),    // left
            CropSettings::new(66.666, 16.666, 16.666, full_width, full_height, grids_x, grids_y), // center
            CropSettings::new(66.666, 33.333, 16.666, full_width, full_height, grids_x, grids_y), // right
            CropSettings::new(66.666, 0.0, 33.333, full_width, full_height, grids_x, grids_y),    // bottom left
            CropSettings::new(66.666, 16.666, 33.333, full_width, full_height, grids_x, grids_y), // bottom
            CropSettings::new(66.666, 33.333, 33.333, full_width, full_height, grids_x, grids_y), // bottom right
        ];

        frame_crops
    }
}

#[derive(Debug)]
struct ImageData {
    tiles: Vec<ImageTile>,
}

#[derive(Debug)]
struct ImageTile {
    colors: Vec<Color>,
}

#[derive(Debug)]
struct FrameMatch {
    tile_index: u64,
    frame_index: u64,
    
    crop_resize: f64,
    crop_pos_x: f64,
    crop_pos_y: f64,
}

const GRIDS_X: u32 = 3;
const GRIDS_Y: u32 = 3;

const MOSAIC_TILES: u32 = 40;

const RESIZE_WIDTH: u32 = 960;
const RESIZE_HEIGHT: u32 = 540;
const BYTES_PER_PIXEL: u32 = 3;

fn main() {
    let video_path = "videos/sample.mp4";
    let image_path = "images/test-image.jpeg";

    let meta_data = extract_video_meta_data(video_path)
        .expect("Error extracting video meta data");

    println!("Video: {}x{} at {}fps", meta_data.width, meta_data.height, meta_data.frame_rate);

    let data_file = &format!("{video_path}.pmg");

    if !exists(data_file).unwrap() {
        println!("Building database...");
        generate_database(video_path, data_file, &meta_data)
            .expect("Failed to load database");
    }

    println!("Loading Database...");
    let data = load_database(data_file)
        .expect("Failed to load database");
    println!("    Loaded Data. {} frames processed!", data.len().to_formatted_string(&Locale::en));


    println!("Calculating image colors...");
    let image_data = calculate_image_colors(image_path)
        .expect("Failed to load image");
    println!("    Image color data processed");


    println!("Searching nearest matches...");
    let total_tiles = MOSAIC_TILES * MOSAIC_TILES;
    let mut selected_frames: Vec<FrameMatch> = Vec::with_capacity(total_tiles as usize);

    for tile_y in 0..MOSAIC_TILES {
        for tile_x in 0..MOSAIC_TILES {
            let tile_index = (tile_y * MOSAIC_TILES + tile_x) as usize;
            let candidate = &image_data.tiles[tile_index];

            let frame_match = find_nearest_color(&data, &candidate, tile_index as u64);
            
            selected_frames.push(frame_match);

            let tile_number = tile_index + 1;
            print!("\r    Matched tile {tile_number}/{total_tiles}");
            std::io::stdout().flush().unwrap();
        }
    }
    println!("");
    println!("    Done!");

    println!("Generating Mosaic...");
    generate_mosaic(video_path, &selected_frames, &meta_data)
        .expect("Error generating Mosaic");
}

fn generate_mosaic(video_path: &str, selected_frames: &[FrameMatch], meta_data: &VideoMetaData) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new("ffmpeg")
        .args([
            //"-hwaccel", "videotoolbox", // THIS MAKES IT RUN SLOWER
            "-i", video_path,

            "-vf", &format!("scale={RESIZE_WIDTH}:{RESIZE_HEIGHT}:flags=area"),

            // No audio/subtitles/data output
            "-an",
            "-sn",
            "-dn",

            // Raw RGB pixels
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",

            // Output to stdout
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let frame_size = RESIZE_WIDTH * RESIZE_HEIGHT * BYTES_PER_PIXEL;

    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = vec![0u8; frame_size as usize];

    let mut frame_index: u64 = 0;
    let total_frames = meta_data.total_frames;

    let cell_width: u32 = 480;
    let cell_height: u32 = 270;

    let full_image_width = cell_width * MOSAIC_TILES;
    let full_image_height = cell_height * MOSAIC_TILES;
    let mut canvas: ImageBuffer<Rgb<u8>, Vec<u8>> = RgbImage::new(full_image_width, full_image_height);

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                let percentage_complete = (frame_index as f64 / total_frames as f64) * 100.0;
                print!("\r    Frame: {}/{} - ({:.2}%)",
                    frame_index.to_formatted_string(&Locale::en),
                    total_frames.to_formatted_string(&Locale::en),
                    percentage_complete);
                std::io::stdout().flush().unwrap();

                let matches = selected_frames.iter()
                    .filter(|frame_match| frame_match.frame_index == frame_index);

                for matched in matches {
                    let pixels = buffer.to_vec();
                    let mut image = RgbImage::from_raw(RESIZE_WIDTH, RESIZE_HEIGHT, pixels).unwrap();

                    if matched.crop_resize < 100.0 {
                        let pos_x = f64::round((RESIZE_WIDTH as f64 / 100.0) * matched.crop_pos_x) as u32;
                        let pos_y = f64::round((RESIZE_HEIGHT as f64 / 100.0) * matched.crop_pos_y) as u32;

                        let cropped_width = f64::round( (RESIZE_WIDTH as f64 / 100.0) * matched.crop_resize) as u32;
                        let cropped_height = f64::round( (RESIZE_HEIGHT as f64 / 100.0) * matched.crop_resize) as u32;

                        image = imageops::crop(&mut image, pos_x, pos_y, cropped_width, cropped_height).to_image();
                    }

                    let resized = imageops::resize(&image, cell_width, cell_height, FilterType::Triangle);
    
                    let row = matched.tile_index as u32 / MOSAIC_TILES;
                    let col = matched.tile_index as u32 % MOSAIC_TILES;

                    canvas.copy_from(&resized, col * cell_width, row * cell_height).unwrap();
                }
 
                frame_index += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => {
                panic!("Failed reading ffmpeg output: {error}");
            }
        }
    }

    println!("");
    println!("    Done");

    let status = child.wait().unwrap();

    if !status.success() {
        panic!("ffmpeg failed!");
    }

    canvas.save("images/test-output.png").unwrap();

    Ok(())
}

const RED_BIAS: u64 = 3;
const GREEN_BIAS: u64 = 6;
const BLUE_BIAS: u64 = 1;

fn find_nearest_color(database: &HashMap<u64, FrameData>, candidate: &ImageTile, tile_index: u64) -> FrameMatch {
    let mut nearest_match = FrameMatch {
        tile_index: tile_index,
        frame_index: 0,
        crop_resize: 100.0,
        crop_pos_x: 0.0,
        crop_pos_y: 0.0,
    };
    let mut smallest_distance = std::u64::MAX;

    for (frame_index, frame) in database {
        let full_frame_dist = check_distance(&frame.full_frame, candidate);

        if full_frame_dist < smallest_distance {
			nearest_match = FrameMatch {
                tile_index: tile_index,
                frame_index: *frame_index,
                crop_resize: 100.0,
                crop_pos_x: 0.0,
                crop_pos_y: 0.0,
            };
			smallest_distance = full_frame_dist;
		}

        for crop in &frame.crops {
            let crop_distance = check_distance(&crop.colors, candidate);

            if crop_distance < smallest_distance {
                nearest_match = FrameMatch {
                    tile_index: tile_index,
                    frame_index: *frame_index,
                    crop_resize: crop.resize_percentage,
                    crop_pos_x: crop.pos_x_percentage,
                    crop_pos_y: crop.pos_y_percentage,
                };
                smallest_distance = crop_distance;
            }
        }
    }

    nearest_match
}

fn check_distance(frame_colors: &[Color], candidate: &ImageTile) -> u64 {
    let mut distance: u64 = 0;

    for i in 0..candidate.colors.len() {
        let frame_color = &frame_colors[i];
        let image_color = &candidate.colors[i];

        let red_dist = frame_color.r.abs_diff(image_color.r) as u64;
        let green_dist = frame_color.g.abs_diff(image_color.g) as u64;
        let blue_dist = frame_color.b.abs_diff(image_color.b) as u64;

        distance +=
            (RED_BIAS * red_dist * red_dist) +
            (GREEN_BIAS * green_dist * green_dist) +
            (BLUE_BIAS * blue_dist * blue_dist);
    }

    return distance;
}

fn generate_database(video_path: &str, data_file: &str, meta_data: &VideoMetaData) -> Result<(), Box<dyn Error>> {
    //let filter = "select='eq(n,0)+eq(n,2)+eq(n,36)+eq(n,206)+eq(n,2060)+eq(n,2069)+eq(n,12345)'";

    let frame_crops = CropSettings::all_crops(RESIZE_WIDTH, RESIZE_HEIGHT, GRIDS_X, GRIDS_Y);

    let mut child = Command::new("ffmpeg")
        .args([
            //"-hwaccel", "videotoolbox", // THIS MAKES IT RUN SLOWER
            "-i", video_path,

            // UNCOMMENT TO USE FILTER ABOVE
            // "-vf", filter,
            // "-vsync", "0",

            "-vf", &format!("scale={RESIZE_WIDTH}:{RESIZE_HEIGHT}:flags=area"),

            // No audio/subtitles/data output
            "-an",
            "-sn",
            "-dn",

            // Raw RGB pixels
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",

            // Output to stdout
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let frame_size = RESIZE_WIDTH * RESIZE_HEIGHT * BYTES_PER_PIXEL;

    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = vec![0u8; frame_size as usize];

    let mut frame_number: u64 = 0;
    let total_frames = meta_data.total_frames;

    let mut data = File::create(data_file).unwrap();

    let mut timer = Instant::now();
    let output_frame_interval = 123;

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                process_frame(frame_number, &frame_crops, &buffer, RESIZE_WIDTH, RESIZE_HEIGHT, &mut data);
                frame_number += 1;

                if frame_number % output_frame_interval == 0 {
                    let percentage_complete = (frame_number as f64 / total_frames as f64) * 100.0;

                    let elapsed = timer.elapsed().as_secs_f64();
                    let frames_per_second = output_frame_interval as f64 / elapsed;

                    timer = Instant::now();

                    print!("\r    Processed frame: {}/{} - ({:.2}%) - {:.2}fps",
                        frame_number.to_formatted_string(&Locale::en),
                        total_frames.to_formatted_string(&Locale::en),
                        percentage_complete,
                        frames_per_second);
                    std::io::stdout().flush().unwrap();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => {
                panic!("Failed reading ffmpeg output: {error}");
            }
        }
    }

    println!("");
    println!("    Done");

    let status = child.wait().unwrap();

    if !status.success() {
        panic!("ffmpeg failed!");
    }

    Ok(())
}

fn process_frame(frame_number: u64, frame_crops: &[CropSettings], pixels: &[u8], width: u32, height: u32, data: &mut File) {
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
                    let offset = ((y * width + x) * BYTES_PER_PIXEL) as usize;

                    let red = pixels[offset] as u64;
                    total_red += red;

                    let green = pixels[offset + 1] as u64;
                    total_green += green;

                    let blue = pixels[offset + 2] as u64;
                    total_blue += blue;
                }
            }

            let average_red = f64::round(total_red as f64 / total_grid_pixels as f64) as u64;
            let average_green = f64::round(total_green as f64 / total_grid_pixels as f64) as u64;
            let average_blue = f64::round(total_blue as f64 / total_grid_pixels as f64) as u64;

            writeln!(data, "{frame_number} 100 0 0 {average_red},{average_green},{average_blue}").unwrap();
        }
    }

    for crop in frame_crops.iter() {
        let crop_start_x = crop.crop_start_x;
        let crop_start_y = crop.crop_start_y;

        let cropped_grid_width = crop.cropped_grid_width;
        let cropped_grid_height = crop.cropped_grid_height;
        let cropped_total_grid_pixels = crop.cropped_total_grid_pixels;

        let resize = crop.resize_percentage;
        let pos_x = crop.pos_x_percentage;
        let pos_y = crop.pos_y_percentage;

        for grid_y in 0..GRIDS_Y {
            for grid_x in 0..GRIDS_X {
                let start_x = crop_start_x + (cropped_grid_width * grid_x);
                let end_x = start_x + cropped_grid_width;

                let start_y = crop_start_y + (cropped_grid_height * grid_y);
                let end_y = start_y + cropped_grid_height;

                let mut total_red: u64 = 0;
                let mut total_green: u64 = 0;
                let mut total_blue: u64 = 0;

                for x in start_x..end_x {
                    for y in start_y..end_y {
                        let offset = ((y * width + x) * BYTES_PER_PIXEL) as usize;

                        let red = pixels[offset];
                        total_red += red as u64;

                        let green = pixels[offset + 1];
                        total_green += green as u64;

                        let blue = pixels[offset + 2];
                        total_blue += blue as u64;
                    }
                }

                let average_red = f64::round(total_red as f64 / cropped_total_grid_pixels as f64) as u64;
                let average_green = f64::round(total_green as f64 / cropped_total_grid_pixels as f64) as u64;
                let average_blue = f64::round(total_blue as f64 / cropped_total_grid_pixels as f64) as u64;
                
                writeln!(data, "{frame_number} {resize} {pos_x} {pos_y} {average_red},{average_green},{average_blue}").unwrap();
            }
        }
    }

    // let image = mage::RgbImage::from_raw(width as u32, height as u32, pixels.to_vec()).unwrap();
    // image.save(format!("test/{frame_number}.jpg")).unwrap();
}

fn extract_video_meta_data(video_path: &str) -> Result<VideoMetaData, Box<dyn Error>> {
    let meta_ouput = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,nb_frames",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path,
        ])
        .stdout(Stdio::piped())
        .output()
        .unwrap();

    let result = String::from_utf8(meta_ouput.stdout)?;
    
    let meta_items: Vec<&str> = result.split("\n").collect();

    let width: u32 = meta_items[0].parse()?;
    let height: u32 = meta_items[1].parse()?;

    let fps_parts: Vec<&str> = meta_items[2].split("/").collect();
    let fps_first: u32 = fps_parts[0].parse()?;
    let fps_last: u32 = fps_parts[1].parse()?;
    let fps: f64 = fps_first as f64 / fps_last as f64;
    let frame_rate = (fps * 100.0).round() / 100.0;

    let total_frames: u64 = meta_items[3].parse()?;

    let meta_data = VideoMetaData {
        width: width,
        height: height,
        frame_rate: frame_rate,
        total_frames: total_frames,
    };

    Ok(meta_data)
}

fn load_database(data_file: &str) -> Result<HashMap<u64, FrameData>, Box<dyn Error>> {
    let file = File::open(data_file)?;
    let lines = io::BufReader::new(file).lines();

    let mut data: HashMap<u64, FrameData> = HashMap::new();

    let mut current_iframe_ndex: u64 = 0;
    let mut current_frame = FrameData::new(GRIDS_X, GRIDS_Y);

    for line in lines.map_while(Result::ok) {
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 5 {
            eprintln!("Invalid data: {line}");
            break;
        }

        let index: u64 = parts[0].parse()?;
        if index > current_iframe_ndex {
            data.insert(current_iframe_ndex, current_frame);

            current_iframe_ndex = index;
            current_frame = FrameData::new(GRIDS_X, GRIDS_Y);
        }

        let resize_percentage: f64 = parts[1].parse()?;
        let pos_x: f64 = parts[2].parse()?;
        let pos_y: f64 = parts[3].parse()?;

        let rgb: Vec<&str> = parts[4].split(',').collect();

        let r: u8 = rgb[0].parse()?;
        let g: u8 = rgb[1].parse()?;
        let b: u8 = rgb[2].parse()?;

        if resize_percentage == 100.0 {
            current_frame.full_frame.push(Color { r, g, b });
        }
        else {
            let crop = current_frame.crops.iter_mut()
                .find(|crop| {
                    crop.resize_percentage == resize_percentage &&
                    crop.pos_x_percentage == pos_x &&
                    crop.pos_y_percentage == pos_y
                });

            if let Some(crop) = crop {
                crop.colors.push(Color { r, g, b });
            }
            else {
                current_frame.crops.push(FrameCrop {
                    resize_percentage: resize_percentage,
                    pos_x_percentage: pos_x,
                    pos_y_percentage: pos_y,
                    colors: vec![Color { r, g, b }],
                });
            }
        }
    }

    Ok(data)
}

fn calculate_image_colors(image_path: &str) -> Result<ImageData, Box<dyn Error>> {
    let image_file = image::open(image_path)?;
    let image = image_file.to_rgb8();
    let (width, height) = image.dimensions();

    let mut image_data = ImageData { tiles: vec![] };
    
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

            let mut tile_data = ImageTile {
                colors: vec![]
            };
            
            for sub_tile_y in 0..GRIDS_Y {
                for sub_tile_x in 0..GRIDS_X {
                    let start_x = sub_tile_x * sub_tile_width;
                    let end_x = start_x + sub_tile_width;

                    let start_y = sub_tile_y * sub_tile_height;
                    let end_y = start_y + sub_tile_height;

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

            image_data.tiles.push(tile_data);
        }
    }   

    Ok(image_data)
}