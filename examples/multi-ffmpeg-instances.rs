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
use anyhow::Result;

const GRIDS_X: u32 = 4;
const GRIDS_Y: u32 = 4;

const RESIZE_WIDTH: u32 = 1920;
const RESIZE_HEIGHT: u32 = 1080;
const BYTES_PER_PIXEL: u32 = 3;

const NUM_THREADS: u32 = 9;
const FFMPEG_THREADS: u32 = 2;

struct ProcessReport {
    thread_index: u32,
    total_frames_processed: u64,
    average_fps: f64,
    percentage_complete: f64,
}

fn main() {
    let video_path = "videos/mustangs.mp4";
    let use_summed_table_algorithm = true;

    let meta_data = match extract_video_meta_data(video_path) {
        Ok(md) => md,
        Err(err) => {
            panic!("{err}");
        }
    };

    println!("Processing {}x{} video. Duration: {}", meta_data.width, meta_data.height, format_duration(meta_data.duration));
    if use_summed_table_algorithm {
        println!("Using Summed-table method");
    }
    else {
        println!("Using Pixel Array method");
    }
    println!("");

    if meta_data.is_variable_frame_rate {
        panic!("Variable frame rates not supported");
    }

    let timer = Instant::now();

    let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(NUM_THREADS as usize);
    let (tx, rx) = mpsc::channel();

    let frames_per_thread = f64::ceil(meta_data.total_frames as f64 / NUM_THREADS as f64) as u64;

    for thread_index in 0..NUM_THREADS {
        let starting_frame_index = thread_index as u64 * frames_per_thread;
        let ending_frame_index = starting_frame_index + frames_per_thread;
        let seconds_to_target_frame = starting_frame_index as f64 / meta_data.frame_rate;
        let tx: mpsc::Sender<ProcessReport> = tx.clone();

        workers.push(thread::spawn(move || {
            let frame_crops = CropSetting::all_crops(RESIZE_WIDTH, RESIZE_HEIGHT, GRIDS_X, GRIDS_Y).unwrap();

            let temp_file_path = format!("{video_path}_core-{thread_index}_temp.pmg");
            let file = File::create(temp_file_path).unwrap();
            let mut data = BufWriter::new(file);

            let mut child = Command::new("ffmpeg")
                .args([
                    "-threads", &format!("{FFMPEG_THREADS}"),
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

            let average_timer = Instant::now();
            let output_frame_interval = 123;

            let mut integral = IntegralImage::new(RESIZE_WIDTH as u32, RESIZE_HEIGHT as u32);

            loop {
                match stdout.read_exact(&mut buffer) {
                    Ok(()) => {
                        if use_summed_table_algorithm {
                            integral.init(&buffer);
                            process_frame_integral(current_frame_index, &frame_crops, &integral, &mut data);
                        }
                        else {
                            process_frame(current_frame_index, &frame_crops, &buffer, RESIZE_WIDTH, RESIZE_HEIGHT, &mut data);
                        }

                        current_frame_index += 1;

                        if current_frame_index == ending_frame_index {
                            tx.send(ProcessReport {
                                thread_index: thread_index as u32,
                                total_frames_processed: frames_per_thread,
                                average_fps: 0.0,
                                percentage_complete: 100.0,
                            }).unwrap();

                            child.kill().unwrap();
                            break;
                        }

                        if current_frame_index % output_frame_interval == 0 {
                            let total_frames_processed = current_frame_index - starting_frame_index;
                            let percentage_complete = (total_frames_processed as f64 / frames_per_thread as f64) * 100.0;

                            let average_elapsed = average_timer.elapsed().as_secs_f64();
                            let average_fps = total_frames_processed as f64 / average_elapsed;

                            tx.send(ProcessReport {
                                thread_index: thread_index as u32,
                                total_frames_processed: total_frames_processed,
                                average_fps: average_fps,
                                percentage_complete: percentage_complete,
                            }).unwrap();
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                        tx.send(ProcessReport {
                            thread_index: thread_index as u32,
                            total_frames_processed: frames_per_thread,
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

    let mut reports: Vec<Option<ProcessReport>> = Vec::with_capacity(NUM_THREADS as usize);
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

fn process_frame_integral(frame_number: u64, frame_crops: &[CropSetting], integral: &IntegralImage, data: &mut BufWriter<File>) {
    let mut output = String::with_capacity(8192);

    for crop in frame_crops.iter() {
        let resize = crop.resize_percentage;
        let pos_x = crop.pos_x_percentage;
        let pos_y = crop.pos_y_percentage;

        write!(&mut output, "{frame_number} {resize} {pos_x} {pos_y}").unwrap();

        for tile in crop.tiles.iter() {
            let [average_red, average_green, average_blue] = integral
                .average_rect(tile.start_x, tile.start_y, tile.end_x, tile.end_y);
                
            write!(&mut output, " {average_red},{average_green},{average_blue}").unwrap();
        }

        writeln!(&mut output).unwrap();
    }

    data.write_all(output.as_bytes()).unwrap();
}

fn process_frame(frame_number: u64, frame_crops: &[CropSetting], pixels: &[u8], width: u32, _: u32, data: &mut BufWriter<File>) {
    let mut output = String::with_capacity(8192);
    
    for crop in frame_crops.iter() {
        let resize = crop.resize_percentage;
        let pos_x = crop.pos_x_percentage;
        let pos_y = crop.pos_y_percentage;

        write!(&mut output, "{frame_number} {resize} {pos_x} {pos_y}").unwrap();

        for tile in crop.tiles.iter() {
            let mut total_red: u32 = 0;
            let mut total_green: u32 = 0;
            let mut total_blue: u32 = 0;

            for y in tile.start_y..tile.end_y {
                let row_start = ((y * width + tile.start_x) * BYTES_PER_PIXEL) as usize;
                let row_end = ((y * width + tile.end_x) * BYTES_PER_PIXEL) as usize;

                for pixel in pixels[row_start..row_end].chunks_exact(3) {
                    total_red += pixel[0] as u32;
                    total_green += pixel[1] as u32;
                    total_blue += pixel[2] as u32;
                }
            }

            let average_red = f64::round(total_red as f64 / tile.total_pixels as f64) as u8;
            let average_green = f64::round(total_green as f64 / tile.total_pixels as f64) as u8;
            let average_blue = f64::round(total_blue as f64 / tile.total_pixels as f64) as u8;
            
            write!(&mut output, " {average_red},{average_green},{average_blue}").unwrap();
        }

        writeln!(&mut output).unwrap();
    }

    data.write_all(output.as_bytes()).unwrap();
}

struct IntegralImage {
    sums: Vec<[u32; 3]>,
    stride: u32,
    width: u32,
    height: u32,
}

impl IntegralImage {
    fn new(width: u32, height: u32) -> Self {
        let stride = width + 1;
        let sums = vec![[0u32; 3]; ((width + 1) * (height + 1)) as usize];

        Self {
            sums,
            stride,
            width,
            height,
        }
    }

    fn init(&mut self, pixels: &[u8]) {
        for y in 0..self.height {
            let mut row_red: u32 = 0;
            let mut row_green: u32 = 0;
            let mut row_blue: u32 = 0;

            let src_row = y * self.width * 3;
            let dst_row = (y + 1) * self.stride;
            let prev_row = y * self.stride;

            for x in 0..self.width {
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

const RATIO_16_9 : f64 = 16.0 / 9.0;

pub struct CropSetting {
    pub resize_percentage: f64,
    pub pos_x_percentage: f64,
    pub pos_y_percentage: f64,

    pub tiles: Vec<CropTile>,
}

pub struct CropTile {
    pub start_x: u32,
    pub end_x: u32,
    pub start_y: u32,
    pub end_y: u32,

    pub total_pixels: u32,
}

impl CropSetting {
    fn new(resize: f64, pos_x: f64, pos_y: f64, frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Self {
        let cropped_width = f64::round((frame_width as f64 / 100.0) * resize) as u32;
        let cropped_height = f64::round(cropped_width as f64 / RATIO_16_9) as u32;

        let crop_start_x = f64::round((frame_width as f64 / 100.0) * pos_x) as u32;
        let crop_start_y = f64::round((frame_height as f64 / 100.0) * pos_y) as u32;

        let cropped_grid_width = f64::round(cropped_width as f64 / tiles_x as f64) as u32;
        let cropped_grid_height = f64::round(cropped_height as f64 / tiles_y as f64) as u32;

        let mut tiles: Vec<CropTile> = Vec::with_capacity((tiles_x * tiles_y) as usize);

        for grid_y in 0..tiles_y {
            for grid_x in 0..tiles_x {
                let start_x = crop_start_x + (cropped_grid_width * grid_x);
                let end_x = cmp::min(start_x + cropped_grid_width, frame_width);

                let start_y = crop_start_y + (cropped_grid_height * grid_y);
                let end_y = cmp::min(start_y + cropped_grid_height, frame_height);

                let tile_width = end_x - start_x;
                let tile_height = end_y - start_y;
                let total_pixels = tile_width * tile_height;

                tiles.push(CropTile { start_x, end_x, start_y, end_y, total_pixels });
            }
        }

        Self {
            resize_percentage: resize,
            pos_x_percentage: pos_x,
            pos_y_percentage: pos_y,

            tiles: tiles,
        }
    }

    pub fn all_crops(frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Result<Vec<Self>> {
        let aspect_ratio = frame_width as f64 / frame_height as f64;
        let aspect_ratio = (aspect_ratio * 100.0).round() / 100.0;

        if aspect_ratio == 1.78 {
            Ok(CropSetting::get_16x9(frame_width, frame_height, tiles_x, tiles_y))
        }
        else {
            Err(anyhow::format_err!("`{aspect_ratio}` aspect ratio videos aren't supported"))
        }
    }

    fn get_16x9(frame_width: u32, frame_height: u32, tiles_x: u32, tiles_y: u32) -> Vec<Self> {
        vec![
            // full frame
            Self::new(100.0, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y), // full frame

            // 50% crops
            Self::new(50.0, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),   // top left
            Self::new(50.0, 25.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),  // top
            Self::new(50.0, 50.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),  // top right
            Self::new(50.0, 0.0, 25.0, frame_width, frame_height, tiles_x, tiles_y),  // left
            Self::new(50.0, 25.0, 25.0, frame_width, frame_height, tiles_x, tiles_y), // center
            Self::new(50.0, 50.0, 25.0, frame_width, frame_height, tiles_x, tiles_y), // right
            Self::new(50.0, 0.0, 50.0, frame_width, frame_height, tiles_x, tiles_y),  // bottom left
            Self::new(50.0, 25.0, 50.0, frame_width, frame_height, tiles_x, tiles_y), // bottom
            Self::new(50.0, 50.0, 50.0, frame_width, frame_height, tiles_x, tiles_y), // bottom right

            // 50% inner crops
            Self::new(50.0, 12.5, 12.5, frame_width, frame_height, tiles_x, tiles_y), // inner top left
            Self::new(50.0, 37.5, 12.5, frame_width, frame_height, tiles_x, tiles_y), // inner top right
            Self::new(50.0, 12.5, 37.5, frame_width, frame_height, tiles_x, tiles_y), // inner bottom left
            Self::new(50.0, 37.5, 37.5, frame_width, frame_height, tiles_x, tiles_y), // inner bottom right

            // 66.666% crops
            Self::new(66.666, 0.0, 0.0, frame_width, frame_height, tiles_x, tiles_y),       // top left
            Self::new(66.666, 16.666, 0.0, frame_width, frame_height, tiles_x, tiles_y),    // top
            Self::new(66.666, 33.333, 0.0, frame_width, frame_height, tiles_x, tiles_y),    // top right
            Self::new(66.666, 0.0, 16.666, frame_width, frame_height, tiles_x, tiles_y),    // left
            Self::new(66.666, 16.666, 16.666, frame_width, frame_height, tiles_x, tiles_y), // center
            Self::new(66.666, 33.333, 16.666, frame_width, frame_height, tiles_x, tiles_y), // right
            Self::new(66.666, 0.0, 33.333, frame_width, frame_height, tiles_x, tiles_y),    // bottom left
            Self::new(66.666, 16.666, 33.333, frame_width, frame_height, tiles_x, tiles_y), // bottom
            Self::new(66.666, 33.333, 33.333, frame_width, frame_height, tiles_x, tiles_y), // bottom right
        ]
    }
}