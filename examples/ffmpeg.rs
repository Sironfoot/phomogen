use std::error::Error;
use std::fs::{File, exists};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::{self, BufRead, Read, Write};

use image::GenericImageView;

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
    top_left: Vec<Color>,
    top_right: Vec<Color>,
    bottom_left: Vec<Color>,
    bottom_right: Vec<Color>,
    center_frame: Vec<Color>,
}

impl FrameData {
    fn new(grid_x: u32, grid_y: u32) -> FrameData {
        let grid_tiles = (grid_x * grid_y) as usize;

        FrameData {
            full_frame: Vec::with_capacity(grid_tiles),
            top_left: Vec::with_capacity(grid_tiles),
            top_right: Vec::with_capacity(grid_tiles),
            bottom_left: Vec::with_capacity(grid_tiles),
            bottom_right: Vec::with_capacity(grid_tiles),
            center_frame: Vec::with_capacity(grid_tiles),
        }
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

const GRIDS_X: u32 = 3;
const GRIDS_Y: u32 = 3;

const MOSAIC_TILES: u32 = 40;

const RESIZE_WIDTH: u32 = 960;
const RESIZE_HEIGHT: u32 = 540;
const BYTES_PER_PIXEL: u32 = 3;

fn main() {
    let video_path = "sample.mp4";
    let image_path = "test-image.jpeg";

    let meta_data = extract_video_meta_data(video_path)
        .expect("Error extracting video meta data");

    println!("Video: {}x{} at {}fps", meta_data.width, meta_data.height, meta_data.frame_rate);

    let data_file = &format!("{video_path}.dat");

    if !exists(data_file).unwrap() {
        println!("Building database...");
        generate_database(video_path, data_file, &meta_data)
            .expect("Failed to load database");
    }

    println!("Loading Database...");
    let data = load_database(data_file)
        .expect("Failed to load database");
    println!("    Loaded Data. {} frames processed!", data.len());


    println!("Calculating image colors...");
    let image_data = calculate_image_colors(image_path)
        .expect("Failed to load image");
    println!("    Image color data processed");

    
}

fn generate_database(video_path: &str, data_file: &str, meta_data: &VideoMetaData) -> Result<(), Box<dyn Error>> {
    //let filter = "select='eq(n,0)+eq(n,2)+eq(n,36)+eq(n,206)+eq(n,2060)+eq(n,2069)+eq(n,12345)'";

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

    let mut data = File::create(data_file).unwrap();

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                process_frame(frame_number, meta_data.total_frames, &buffer, RESIZE_WIDTH, RESIZE_HEIGHT, &mut data);
                frame_number += 1;
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

fn process_frame(frame_number: u64, total_frames: u64, pixels: &[u8], width: u32, height: u32, data: &mut File) {
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

            let average_red = total_red / total_grid_pixels;
            let average_green = total_green / total_grid_pixels;
            let average_blue = total_blue / total_grid_pixels;

            writeln!(data, "{frame_number} ff {average_red},{average_green},{average_blue}").unwrap();
        }
    }

    // process crops
    // top left
    process_cropped_frame(0, 0, "tl", frame_number, pixels, width, height, data);
    // top right
    process_cropped_frame(width / 2, 0, "tr", frame_number, pixels, width, height, data);
    // bottom left
    process_cropped_frame(0, height / 2, "bl", frame_number, pixels, width, height, data);
    // bottom right
    process_cropped_frame(width / 2, height / 2, "br", frame_number, pixels, width, height, data);
    // center
    process_cropped_frame(width / 4, height / 4, "cf", frame_number, pixels, width, height, data);

    let percentage_complete = (frame_number as f64 / total_frames as f64) * 100.0;
    print!("\r    Processed frame: {frame_number}/{total_frames} - ({:.2}%)", percentage_complete);
    std::io::stdout().flush().unwrap();

    // let image = mage::RgbImage::from_raw(width as u32, height as u32, pixels.to_vec()).unwrap();
    // image.save(format!("test/{frame_number}.jpg")).unwrap();
}

fn process_cropped_frame(crop_start_x: u32, crop_start_y: u32, prefix: &str, frame_number: u64, pixels: &[u8], width: u32, height: u32, data: &mut File) {
    let cropped_width = width / 2;
    let cropped_height = height / 2;

    let cropped_grid_width = cropped_width / GRIDS_X;
    let cropped_grid_height = cropped_height / GRIDS_Y;
    let cropped_total_grid_pixels = (cropped_grid_width * cropped_grid_height) as u64;

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

                    let red = pixels[offset] as u64;
                    total_red += red;

                    let green = pixels[offset + 1] as u64;
                    total_green += green;

                    let blue = pixels[offset + 2] as u64;
                    total_blue += blue;
                }
            }

            let average_red = total_red / cropped_total_grid_pixels;
            let average_green = total_green / cropped_total_grid_pixels;
            let average_blue = total_blue / cropped_total_grid_pixels;

            writeln!(data, "{frame_number} {prefix} {average_red},{average_green},{average_blue}").unwrap();
        }
    }
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
        if parts.len() != 3 {
            eprintln!("Invalid data: {line}");
            break;
        }

        let index: u64 = parts[0].parse()?;
        if index > current_iframe_ndex {
            data.insert(current_iframe_ndex, current_frame);

            current_iframe_ndex = index;
            current_frame = FrameData::new(GRIDS_X, GRIDS_Y);
        }

        let frame_type = parts[1];

        let rgb: Vec<&str> = parts[2].split(',').collect();

        let r: u8 = rgb[0].parse()?;
        let g: u8 = rgb[1].parse()?;
        let b: u8 = rgb[2].parse()?;

        match frame_type {
            "ff" => current_frame.full_frame.push(Color { r, g, b }),
            "tl" => current_frame.top_left.push(Color { r, g, b }),
            "tr" => current_frame.top_right.push(Color { r, g, b }),
            "bl" => current_frame.bottom_left.push(Color { r, g, b }),
            "br" => current_frame.bottom_right.push(Color { r, g, b }),
            "cf" => current_frame.center_frame.push(Color { r, g, b }),
            _ => return Err(format!("Unknown frame type: {frame_type}").into()),
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