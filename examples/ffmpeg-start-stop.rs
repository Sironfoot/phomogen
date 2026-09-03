use std::{env, error::Error, io::Read, path::Path, process::{Command, Stdio}, time::Instant};

use image::{RgbImage, imageops};
use image::imageops::{FilterType};
use rand::seq::index::sample;

const TOTAL_FRAMES: u32 = 36_000;
const NUM_FRAMES_TO_EXTRACT: usize = 100;

fn main() -> Result<(), Box<dyn Error>> {
    let num_frames_to_extract = env::args()
        .nth(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(NUM_FRAMES_TO_EXTRACT);

    let total_frames = TOTAL_FRAMES as usize + 1;
    if num_frames_to_extract > total_frames {
        return Err(format!("X cannot exceed {total_frames}").into());
    }

    let mut rng = rand::rng();
    let mut frame_indices: Vec<u32> = sample(&mut rng, total_frames, num_frames_to_extract)
        .into_iter()
        .map(|value| value as u32)
        .collect();
    frame_indices.sort_unstable();

    let video_path = Path::new("./videos/Frame Test 29.97.mov");
    let frame_rate = 29.97002997;

    let frame_width: u32 = 1920;
    let frame_height: u32 = 1080;

    let mut timer = Instant::now();

    start_top_instance(&frame_indices, video_path, frame_width, frame_height, frame_rate);
    println!("Start/Stop strategy took: {:.2} seconds", timer.elapsed().as_secs_f64());

    timer = Instant::now();
    single_instance(&frame_indices, video_path,frame_width, frame_height, total_frames as u32, frame_rate);
    println!("Single Instance strategy took: {:.2} seconds", timer.elapsed().as_secs_f64());

    Ok(())
}

fn start_top_instance(frame_indices: &[u32], video_file: &Path, frame_width: u32, frame_height: u32, frame_rate: f64) {
    let frame_size: u32 = frame_width * frame_height * 3;

    for frame_index in frame_indices {
        let seconds_to_target_frame = *frame_index as f64 / frame_rate;

        let mut child = Command::new("ffmpeg")
            .args([
                //"-hwaccel", "auto", // TODO: need to detect GPU decode is available, fall back to CPU
                "-ss", &format!("{seconds_to_target_frame}"),
                "-i"]).arg(&video_file)
            .args([
                "-frames:v", "1",
                "-vf", &format!("scale={frame_width}:-2:flags=area"),

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

        let mut stdout = child.stdout.take().unwrap();
        let mut buffer = vec![0u8; frame_size as usize];

        loop {
            match stdout.read_exact(&mut buffer) {
                Ok(()) => {
                    let image = RgbImage::from_raw(
                        frame_width, frame_height, buffer.to_vec()).unwrap();

                    let resized = imageops::resize(&image, 960, 540, FilterType::Triangle);
                    resized.save(format!("./test/{}.png", frame_index)).unwrap();
                },
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                },
                Err(error) => {
                    panic!("Failed reading ffmpeg output: {error}");
                }
            }
        }

        let status = child.wait().unwrap();
        if !status.success() {
            panic!("ffmpeg failed!");
        }
    }
}

fn single_instance(frame_indices: &[u32], video_file: &Path, frame_width: u32, frame_height: u32, total_frames: u32, frame_rate: f64) {
    let first_frame_index = frame_indices[0];
    let seconds_to_first_frame = first_frame_index as f64 / frame_rate;

    let mut child = Command::new("ffmpeg")
        .args([
            //"-hwaccel", "videotoolbox", // THIS MAKES IT RUN SLOWER
            "-ss", &format!("{seconds_to_first_frame}"),
            "-i"]).arg(&video_file)
        .args([
            "-vf", &format!("scale={frame_width}:-2:flags=area"),

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

    let frame_size: u32 = frame_width * frame_height * 3;

    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = vec![0u8; frame_size as usize];

    let mut frame_index: u32 = first_frame_index;

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                if frame_indices.contains(&frame_index) {
                    let image = RgbImage::from_raw(
                        frame_width, frame_height, buffer.to_vec()).unwrap();

                    let resized = imageops::resize(&image, 960, 540, FilterType::Triangle);
                    resized.save(format!("./test/{}-x.png", frame_index)).unwrap();
                }

                if frame_index >= total_frames {
                    child.kill().unwrap();
                    break;
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

    let status = child.wait().unwrap();
    if !status.success() {
        panic!("ffmpeg failed!");
    }
}