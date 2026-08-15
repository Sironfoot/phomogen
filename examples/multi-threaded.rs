use std::{cmp, error::Error, fs::{self, File}, io::{self, Read, Write}, process::{Command, Stdio}, thread, time::Instant};

use std::sync::{Arc, Mutex, mpsc::{sync_channel}};

use num_format::{Locale, ToFormattedString};

use std::fmt::Write as FmtWrite;

struct VideoMetaData {
    width: u32,
    height: u32,
    frame_rate: f64,
    total_frames: u64,
}


#[derive(Clone)]
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

fn main() {
    let video_path = "videos/mustangs.mp4";

    let meta_data = extract_video_meta_data(video_path)
            .expect("Error extracting video meta data");

    println!("Video: {}x{} at {}fps", meta_data.width, meta_data.height, meta_data.frame_rate);

    let data_file = &format!("{video_path}.pmg2");

    generate_database(video_path, data_file, &meta_data)
        .expect("Failed to load database");
}

const GRIDS_X: u32 = 4;
const GRIDS_Y: u32 = 4;

const RESIZE_WIDTH: u32 = 960;
const RESIZE_HEIGHT: u32 = 540;
const BYTES_PER_PIXEL: u32 = 3;

const THREAD_COUNT: usize = 8;
const QUEUE_SIZE: usize = 16;

fn generate_database(video_path: &str, data_file: &str, meta_data: &VideoMetaData) -> Result<(), Box<dyn Error>> {
    let frame_crops = CropSettings::all_crops(RESIZE_WIDTH, RESIZE_HEIGHT, GRIDS_X, GRIDS_Y);

    let mut child = Command::new("ffmpeg")
        .args([
            // "-hwaccel", "videotoolbox", // THIS MAKES IT RUN SLOWER
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
    //let buffer = vec![0u8; frame_size as usize];

    let mut frame_index: u64 = 0;
    let total_frames = meta_data.total_frames;

    let (tx, rx) = sync_channel::<(u64, Vec<u8>)>(QUEUE_SIZE);
    let rx = Arc::new(Mutex::new(rx));

    let (free_tx, free_rx) = sync_channel::<Vec<u8>>(QUEUE_SIZE);

    for _ in 0..QUEUE_SIZE {
        free_tx.send(vec![8u8; frame_size as usize]).unwrap();
    }

    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();

    for thread_index in 0..THREAD_COUNT {
        let rx = Arc::clone(&rx);
        let free_tx = free_tx.clone();
        let frame_crops = frame_crops.clone();
        let video_path = String::from(video_path);

        workers.push(thread::spawn(move || {
            let temp_file = format!("{video_path}_core-{thread_index}_temp.pmg");
            let mut data = File::create(temp_file).unwrap();

            loop {
                let result = rx.lock().unwrap().recv();

                let (frame_index, buffer) = match result {
                    Ok(frame) => frame,
                    Err(_) => break,
                };

                process_frame(frame_index, &frame_crops, &buffer, RESIZE_WIDTH, RESIZE_HEIGHT, &mut data);

                free_tx.send(buffer).unwrap();
            }
        }));
    }

    let mut current_timer = Instant::now();
    let average_timer = Instant::now();
    let output_frame_interval = 123;

    loop {
        let mut buffer = free_rx.recv().unwrap();

        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                tx.send((frame_index, buffer)).unwrap();
                frame_index += 1;

                if frame_index % output_frame_interval == 0 {
                    let percentage_complete = (frame_index as f64 / total_frames as f64) * 100.0;

                    let current_elapsed = current_timer.elapsed().as_secs_f64();
                    let current_fps = output_frame_interval as f64 / current_elapsed;
                    current_timer = Instant::now();

                    let average_elapsed = average_timer.elapsed().as_secs_f64();
                    let average_fps = frame_index as f64 / average_elapsed;

                    print!("\r    Processed frame: {}/{} - ({:.2}%) - {:.2} fps (avg: {:.2} fps)",
                        frame_index.to_formatted_string(&Locale::en),
                        total_frames.to_formatted_string(&Locale::en),
                        percentage_complete,
                        current_fps,
                        average_fps);
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

    drop(tx);
    drop(free_tx);

    for worker in workers {
        worker.join().unwrap();
    }

    println!("");
    println!("    Done");

    let status = child.wait().unwrap();

    if !status.success() {
        panic!("ffmpeg failed!");
    }

    let mut data = File::create(data_file).unwrap();
    for thread_index in 0..THREAD_COUNT {
        let temp_file_path = &format!("{video_path}_core-{thread_index}_temp.pmg");
        let mut temp_file = File::open(temp_file_path)?;

        io::copy(&mut temp_file, &mut data)?;

        fs::remove_file(temp_file_path)?;
    }

    Ok(())
}

fn process_frame(frame_number: u64, frame_crops: &[CropSettings], pixels: &[u8], width: u32, height: u32, data: &mut File) {
    let grid_width = width / GRIDS_X;
    let grid_height = height / GRIDS_Y;

    let total_grid_pixels: u64 = (grid_width * grid_height) as u64;

    let mut output = String::new();

    write!(&mut output, "{frame_number} 100 0 0").unwrap();

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

            write!(&mut output, " {average_red},{average_green},{average_blue}").unwrap();
        }
    }

    writeln!(&mut output).unwrap();

    for crop in frame_crops.iter() {
        let crop_start_x = crop.crop_start_x;
        let crop_start_y = crop.crop_start_y;

        let cropped_grid_width = crop.cropped_grid_width;
        let cropped_grid_height = crop.cropped_grid_height;
        let cropped_total_grid_pixels = crop.cropped_total_grid_pixels;

        let resize = crop.resize_percentage;
        let pos_x = crop.pos_x_percentage;
        let pos_y = crop.pos_y_percentage;

        write!(&mut output, "{frame_number} {resize} {pos_x} {pos_y}").unwrap();

        for grid_y in 0..GRIDS_Y {
            for grid_x in 0..GRIDS_X {
                let start_x = crop_start_x + (cropped_grid_width * grid_x);
                let end_x = cmp::min(start_x + cropped_grid_width, width);

                let start_y = crop_start_y + (cropped_grid_height * grid_y);
                let end_y = cmp::min(start_y + cropped_grid_height, height);

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
                
                write!(&mut output, " {average_red},{average_green},{average_blue}").unwrap();
            }
        }

        writeln!(&mut output).unwrap();
    }

    write!(data, "{output}").unwrap();
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