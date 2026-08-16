use std::cmp;
use std::error::Error;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufWriter, Read};
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use num_format::{Locale, ToFormattedString};

const GRIDS_X: u32 = 4;
const GRIDS_Y: u32 = 4;

const RESIZE_WIDTH: u32 = 1920;
const RESIZE_HEIGHT: u32 = 1080;
const BYTES_PER_PIXEL: u32 = 3;

const NUM_THREADS: usize = 9;

struct ProcessReport {
    thread_index: u32,
    total_frames_processed: u64,
    current_fps: f64,
    average_fps: f64,
    percentage_complete: f64,
}

fn main() {
    let video_path = "videos/srr.mp4";

    let meta_data = match extract_video_meta_data(video_path) {
        Ok(md) => md,
        Err(err) => {
            panic!("{err}");
        }
    };

    println!("Processing {}x{} video. Duration: {}", meta_data.width, meta_data.height, format_duration(meta_data.duration));
    println!("");

    if meta_data.is_variable_frame_rate {
        panic!("Variable frame rates not supported");
    }

    let timer = Instant::now();

    let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(NUM_THREADS);
    let (tx, rx) = mpsc::channel();

    let frames_per_thread = f64::ceil(meta_data.total_frames as f64 / NUM_THREADS as f64) as u64;

    for thread_index in 0..NUM_THREADS {
        let starting_frame_index = thread_index as u64 * frames_per_thread;
        let ending_frame_index = starting_frame_index + frames_per_thread;
        let seconds_to_target_frame = starting_frame_index as f64 / meta_data.frame_rate;
        let tx: mpsc::Sender<ProcessReport> = tx.clone();

        workers.push(thread::spawn(move || {
            let frame_crops = CropSettings::all_crops(RESIZE_WIDTH, RESIZE_HEIGHT, GRIDS_X, GRIDS_Y);

            let temp_file_path = format!("{video_path}_core-{thread_index}_temp.pmg");
            let file = File::create(temp_file_path).unwrap();
            let mut data = BufWriter::new(file);

            let mut child = Command::new("ffmpeg")
                .args([
                    "-threads", "2",
                    "-ss", &format!("{seconds_to_target_frame}"),
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
                .spawn()
                .unwrap();

            let frame_size = RESIZE_WIDTH * RESIZE_HEIGHT * BYTES_PER_PIXEL;

            let mut stdout = child.stdout.take().unwrap();
            let mut buffer = vec![0u8; frame_size as usize];

            let mut current_frame_index = starting_frame_index;

            let mut current_timer = Instant::now();
            let average_timer = Instant::now();
            let output_frame_interval = 123;

            loop {
                match stdout.read_exact(&mut buffer) {
                    Ok(()) => {
                        process_frame(current_frame_index, &frame_crops, &buffer, RESIZE_WIDTH, RESIZE_HEIGHT, &mut data);
                        current_frame_index += 1;

                        if current_frame_index == ending_frame_index {
                            tx.send(ProcessReport {
                                thread_index: thread_index as u32,
                                total_frames_processed: frames_per_thread,
                                current_fps: 0.0,
                                average_fps: 0.0,
                                percentage_complete: 100.0,
                            }).unwrap();

                            child.kill().unwrap();
                            break;
                        }

                        if current_frame_index % output_frame_interval == 0 {
                            let total_frames_processed = current_frame_index - starting_frame_index;
                            let percentage_complete = (total_frames_processed as f64 / frames_per_thread as f64) * 100.0;

                            let current_elapsed = current_timer.elapsed().as_secs_f64();
                            let current_fps = output_frame_interval as f64 / current_elapsed;
                            current_timer = Instant::now();

                            let average_elapsed = average_timer.elapsed().as_secs_f64();
                            let average_fps = total_frames_processed as f64 / average_elapsed;

                            tx.send(ProcessReport {
                                thread_index: thread_index as u32,
                                total_frames_processed: total_frames_processed,
                                current_fps: current_fps,
                                average_fps: average_fps,
                                percentage_complete: percentage_complete,
                            }).unwrap();
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                        tx.send(ProcessReport {
                            thread_index: thread_index as u32,
                            total_frames_processed: frames_per_thread,
                            current_fps: 0.0,
                            average_fps: 0.0,
                            percentage_complete: 100.0,
                        }).unwrap();

                        break;
                    }
                    Err(error) => {
                        panic!("Failed reading ffmpeg output: {error}");
                    }
                }
            }

            let _ = child.wait().unwrap();
        }));
    }

    drop(tx);

    let mut reports: Vec<Option<ProcessReport>> = Vec::with_capacity(NUM_THREADS);
    for _ in 0..NUM_THREADS {
        reports.push(None);
    }

    let mut first_output = true;

    for update in rx {
        let index = update.thread_index as usize;
        reports[index] = Some(update);

        if !first_output {
            print!("\x1B[{}A", NUM_THREADS + 2);
        }

        first_output = false;

        let mut total_frames: u64 = 0;
        let mut total_fps: f64 = 0.0;

        for report in &reports {
            if let Some(report) = report.as_ref() {
                total_frames += report.total_frames_processed;
                total_fps += report.average_fps;

                println!("\x1B[2K - Thread {} - {}/{} frames ({:.2}%) - {:.2} fps",
                    report.thread_index,
                    report.total_frames_processed.to_formatted_string(&Locale::en),
                    frames_per_thread.to_formatted_string(&Locale::en),
                    report.percentage_complete,
                    report.average_fps);
            }
            else {
                println!("\x1B[2K - Thread {}", index);
            }
        }

        println!("\x1B[2K");

        if reports.iter().all(|r| r.is_some()) {
            println!("\x1B[2K - Total: {}/{} frames - {:.2} fps",
                total_frames.to_formatted_string(&Locale::en),
                meta_data.total_frames.to_formatted_string(&Locale::en),
                total_fps);
        }
        else {
            println!("\x1B[2K - Total: ");
        }
    }

    for worker in workers {
        worker.join().unwrap();
    }

    let data_file = &format!("{video_path}.pmg");
    let mut data = File::create(data_file).unwrap();
    for thread_index in 0..NUM_THREADS {
        let temp_file_path = &format!("{video_path}_core-{thread_index}_temp.pmg");
        let mut temp_file = File::open(temp_file_path).unwrap();

        std::io::copy(&mut temp_file, &mut data).unwrap();

        std::fs::remove_file(temp_file_path).unwrap();
    }

    let total_elapsed = timer.elapsed();

    println!("");
    println!("Finished: Time: {}", format_duration(total_elapsed));
}

fn process_frame(frame_number: u64, frame_crops: &[CropSettings], pixels: &[u8], width: u32, height: u32, data: &mut BufWriter<File>) {
    let grid_width = width / GRIDS_X;
    let grid_height = height / GRIDS_Y;

    let total_grid_pixels: u32 = (grid_width * grid_height) as u32;

    let mut output = String::with_capacity(8192);

    write!(&mut output, "{frame_number} 100 0 0").unwrap();

    for grid_y in 0..GRIDS_Y {
        for grid_x in 0..GRIDS_X {
            let start_x = grid_width * grid_x;
            let end_x = start_x + grid_width;

            let start_y = grid_height * grid_y;
            let end_y = start_y + grid_height;

            let mut total_red: u32 = 0;
            let mut total_green: u32 = 0;
            let mut total_blue: u32 = 0;

            for y in start_y..end_y {
                let row_start = ((y * width + start_x) * BYTES_PER_PIXEL) as usize;
                let row_end = ((y * width + end_x) * BYTES_PER_PIXEL) as usize;

                for pixel in pixels[row_start..row_end].chunks_exact(3) {
                    total_red += pixel[0] as u32;
                    total_green += pixel[1] as u32;
                    total_blue += pixel[2] as u32;
                }
            }

            let average_red = f64::round(total_red as f64 / total_grid_pixels as f64) as u8;
            let average_green = f64::round(total_green as f64 / total_grid_pixels as f64) as u8;
            let average_blue = f64::round(total_blue as f64 / total_grid_pixels as f64) as u8;

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

                let mut total_red: u32 = 0;
                let mut total_green: u32 = 0;
                let mut total_blue: u32 = 0;

                for y in start_y..end_y {
                    let row_start = ((y * width + start_x) * BYTES_PER_PIXEL) as usize;
                    let row_end = ((y * width + end_x) * BYTES_PER_PIXEL) as usize;

                    for pixel in pixels[row_start..row_end].chunks_exact(3) {
                        total_red += pixel[0] as u32;
                        total_green += pixel[1] as u32;
                        total_blue += pixel[2] as u32;
                    }
                }

                let average_red = f64::round(total_red as f64 / cropped_total_grid_pixels as f64) as u8;
                let average_green = f64::round(total_green as f64 / cropped_total_grid_pixels as f64) as u8;
                let average_blue = f64::round(total_blue as f64 / cropped_total_grid_pixels as f64) as u8;
                
                write!(&mut output, " {average_red},{average_green},{average_blue}").unwrap();
            }
        }

        writeln!(&mut output).unwrap();
    }

    data.write_all(output.as_bytes()).unwrap();
}

#[derive(Debug)]
struct VideoMetaData {
    width: u32,
    height: u32,
    frame_rate: f64,
    is_variable_frame_rate: bool,
    total_frames: u64,
    duration: Duration
}

fn extract_video_meta_data(video_path: &str) -> Result<VideoMetaData, Box<dyn Error>> {
    let meta_ouput = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,avg_frame_rate,nb_frames",
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

    let frame_rate_raw = meta_items[2];
    let frame_rate = get_frame_rate(meta_items[2])?;
    let average_frame_rate_raw = meta_items[3];

    let is_variable_frame_rate = frame_rate_raw != average_frame_rate_raw;

    let total_frames: u64 = meta_items[4].parse()?;
    let duration_secs = f64::round(total_frames as f64 / frame_rate) as u64;

    let meta_data = VideoMetaData {
        width: width,
        height: height,
        frame_rate: frame_rate,
        is_variable_frame_rate: is_variable_frame_rate,
        total_frames: total_frames,
        duration: Duration::from_secs(duration_secs),
    };

    Ok(meta_data)
}

fn get_frame_rate(input: &str) -> Result<f64, Box<dyn Error>> {
    let fps_parts: Vec<&str> = input.split("/").collect();
    let fps_first: u32 = fps_parts[0].parse()?;
    let fps_last: u32 = fps_parts[1].parse()?;
    let fps: f64 = fps_first as f64 / fps_last as f64;
    let frame_rate = (fps * 100.0).round() / 100.0;

    Ok(frame_rate)
}

const SECONDS_PER_HOUR: f64 = 3600.0;

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs() as f64;

    let hours = f64::floor(total_seconds / SECONDS_PER_HOUR) as u32;
    let minutes = f64::floor((total_seconds % SECONDS_PER_HOUR) / 60.0) as u32;
    let seconds = f64::floor(total_seconds % 60.0) as u32;

    format!("{hours}h {minutes}m {seconds}s")
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