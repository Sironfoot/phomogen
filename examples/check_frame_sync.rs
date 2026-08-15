use std::{error::Error, io::Read, process::{Command, Stdio}, time::Duration};

const RESIZE_WIDTH: u32 = 1920;
const RESIZE_HEIGHT: u32 = 1080;
const BYTES_PER_PIXEL: u32 = 3;

const NUM_THREADS: u32 = 8;

fn main() {
    let video_path = "videos/mustangs-constant.mp4";

    let meta_data = match extract_video_meta_data(video_path) {
        Ok(md) => md,
        Err(err) => {
            panic!("{err}");
        }
    };
        
    println!("{}x{}", meta_data.width, meta_data.height);
    println!("{} fps", meta_data.frame_rate);
    println!("{} avg. fps", meta_data.average_frame_rate);
    println!("{} total frames", meta_data.total_frames);
    println!("Duration: {}", format_duration(meta_data.duration));
    // println!("");
    // println!("Raw Info:");
    // println!("{}", meta_data.raw_info);

    let mut frames_to_seek: Vec<u64> = vec![];

    for thread_index in 0..NUM_THREADS {
        let frames_per_thread = f64::ceil(meta_data.total_frames as f64 / NUM_THREADS as f64) as u64;
        let starting_frame_index = (thread_index as u64 * frames_per_thread) + 180;

        println!("Thread: {thread_index}");
        println!("Grabbing frame directly...");
        grab_frame_at(starting_frame_index, meta_data.frame_rate, video_path)
            .expect("Failed to grab frame");
        println!("Done!");
        println!("");

        frames_to_seek.push(starting_frame_index);
    }

    println!("Seeking to frames...");
    seek_frames(&frames_to_seek, video_path)
        .expect("Failed to seek frame");
    println!("Done!");
}

fn grab_frame_at(frame_index: u64, frame_rate: f64, video_path: &str) -> Result<(), Box<dyn Error>> {
    let seconds_to_target_frame = frame_index as f64 / frame_rate;

    let mut child = Command::new("ffmpeg")
        .args([
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
        .spawn()?;

    let frame_size = RESIZE_WIDTH * RESIZE_HEIGHT * BYTES_PER_PIXEL;

    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = vec![0u8; frame_size as usize];

    let mut current_frame = frame_index;
    let frame_to_grab = frame_index;

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                if current_frame == frame_to_grab {
                    let image = image::RgbImage::from_raw(RESIZE_WIDTH, RESIZE_HEIGHT, buffer.to_vec()).unwrap();
                    image.save(format!("images/frame_test_{current_frame}_jump.jpeg")).unwrap();

                    child.kill().unwrap();
                    break;
                }

                current_frame += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => {
                panic!("Failed reading ffmpeg output: {error}");
            }
        }
    }

    Ok(())
}

fn seek_frames(frame_indexes: &[u64], video_path: &str) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new("ffmpeg")
        .args([
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

    let mut current_frame: u64 = 0;
    let last_index = frame_indexes.iter().max().unwrap();

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                if frame_indexes.contains(&current_frame) {
                    let image = image::RgbImage::from_raw(RESIZE_WIDTH, RESIZE_HEIGHT, buffer.to_vec()).unwrap();
                    image.save(format!("images/frame_test_{current_frame}_seek.jpeg")).unwrap();
                }

                current_frame += 1;

                if current_frame > *last_index {
                    child.kill().unwrap();
                    break;
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

    Ok(())
}

#[derive(Debug)]
struct VideoMetaData {
    width: u32,
    height: u32,
    frame_rate: f64,
    average_frame_rate: f64,
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

    let frame_rate = get_frame_rate(meta_items[2])?;
    let average_frame_rate = get_frame_rate(meta_items[3])?;

    let total_frames: u64 = meta_items[4].parse()?;
    let duration_secs = f64::round(total_frames as f64 / average_frame_rate) as u64;

    let meta_data = VideoMetaData {
        width: width,
        height: height,
        frame_rate: frame_rate,
        average_frame_rate: average_frame_rate,
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