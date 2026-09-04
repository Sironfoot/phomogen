use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use image::{RgbImage, imageops};
use rand::seq::index::sample;
use image::imageops::{FilterType};

use anyhow::Result;

const NUM_FRAMES_TO_EXTRACT: usize = 100;

fn main() {
    let video_path = PathBuf::from("./videos/iphone-vfr-test/IMG_0870.MOV");
    //let frame_rate = 30.0;
    let frame_width: u32 = 1920;
    let frame_height: u32 = 1080;

    // let timer = Instant::now();
    // let frame_timings_slow = get_frame_timings_slow(&video_path).expect("Error getting slow frame times");
    // println!("{:.2} seconds elapsed to get slow frame timings", timer.elapsed().as_secs_f64());

    // let timer = Instant::now();
    // let frame_timings_fast = get_frame_timings_fast(&video_path).expect("Error getting fast frame times");
    // println!("{:.2} seconds elapsed to get fast frame timings", timer.elapsed().as_secs_f64());

    // assert_eq!(frame_timings_slow.len(), frame_timings_fast.len(), "Num frames not the same");

    // for i in 0..frame_timings_slow.len() {
    //     let slow_timing = frame_timings_slow[i];
    //     let fast_timing = frame_timings_fast[i];

    //     assert_eq!(slow_timing, fast_timing, "Frame timing at index {i} are not the same")
    // }

    // println!("All frame timings match!");

    let frame_timings = get_frame_timings_fast(&video_path).expect("Error getting fast frame times");

    let total_frames = frame_timings.len() - 1 as usize;

    let mut rng = rand::rng();
    let mut frame_indices: Vec<u32> = sample(&mut rng, total_frames, NUM_FRAMES_TO_EXTRACT)
        .into_iter()
        .map(|value| value as u32)
        .collect();

    frame_indices.sort_unstable();

    // frame_indices.clear();
    // for i in 0..frame_timings.len() {
    //     frame_indices.push(i as u32);
    // }

    let timer = Instant::now();
    start_top_instance(&frame_indices, &frame_timings, &video_path, frame_width, frame_height);
    println!("Start/Stop took: {:.2} seconds", timer.elapsed().as_secs_f64());

    let timer = Instant::now();
    single_instance(&frame_indices, &frame_timings, &video_path, frame_width, frame_height, frame_timings.len() as u32);
    println!("Single Instance took: {:.2} seconds", timer.elapsed().as_secs_f64());

}

fn get_frame_timings_slow(video_path: &Path) -> Result<Vec<f64>> {
    let mut frame_timings: Vec<f64> = Vec::new();

    let mut child = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_frames",
            "-show_entries", "frame=best_effort_timestamp_time",
            "-of", "csv=p=0"
        ])
        .arg(video_path)
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = line.unwrap();

        let mut parts = line.split(',');

        // let Some(frame_index) = parts.next().and_then(|v| v.parse::<u32>().ok()) else {
        //     panic!("could not extract frame index");
        // };
    
        let Some(timing) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
            panic!("could not extract frame timing");
        };

        frame_timings.push(timing);
    }

    let _ = child.wait()?;

    Ok(frame_timings)
}

fn get_frame_timings_fast(video_path: &Path) -> Result<Vec<f64>> {
    let mut frame_timings: Vec<f64> = Vec::new();

    let mut child = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_packets",
            "-show_entries", "packet=pts_time",
            "-of", "csv=p=0"
        ])
        .arg(video_path)
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = line.unwrap();

        let timing = line.parse::<f64>().unwrap();
        frame_timings.push(timing);
    }

    frame_timings.sort_by(|a, b| a.total_cmp(b));

    Ok(frame_timings)
}

fn start_top_instance(frame_indices: &[u32], frame_timings: &[f64], video_file: &Path, frame_width: u32, frame_height: u32) {
    let frame_size: u32 = frame_width * frame_height * 3;

    const PRE_ROLL: f64 = 1.1;

    for frame_index in frame_indices {
        let seconds_to_target_frame = frame_timings[*frame_index as usize];
        
        let coarse_seek = (seconds_to_target_frame - PRE_ROLL).max(0.0);
        let fine_seek = seconds_to_target_frame - coarse_seek;

        let mut child = Command::new("ffmpeg")
            .args([
                //"-hwaccel", "auto", // TODO: need to detect GPU decode is available, fall back to CPU
                "-ss", &format!("{coarse_seek}"),
                "-i"]).arg(&video_file)
            .args([
                "-ss", &format!("{fine_seek}"),
                "-frames:v", "1",
                "-vf", &format!("scale={frame_width}:-2:flags=area"),

                // No audio/subtitles/data output
                "-an",
                "-sn",
                "-dn",
                "-fps_mode", "passthrough",

                // Raw RGB pixel
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

fn single_instance(frame_indices: &[u32], frame_timings: &[f64], video_file: &Path, frame_width: u32, frame_height: u32, total_frames: u32) {
    let first_frame_index = frame_indices[0];
    let seconds_to_first_frame = frame_timings[first_frame_index as usize];

    let mut child = Command::new("ffmpeg")
        .args([
            //"-hwaccel", "videotoolbox", // THIS MAKES IT RUN SLOWER
            "-ss", &format!("{seconds_to_first_frame}"),
            "-i"]).arg(&video_file)
        .args([
            "-vf", &format!("scale={frame_width}:-2:flags=area"),
            // "-fps_mode", "vfr",
            // "-r", "30",

            // No audio/subtitles/data output
            "-an",
            "-sn",
            "-dn",
            "-fps_mode", "passthrough",

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

    let _ = child.wait().unwrap();
}