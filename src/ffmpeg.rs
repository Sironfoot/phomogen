use std::{process::{Command, Stdio}, time::Duration};

use anyhow::Result;

pub struct VideoMetaData {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub is_variable_frame_rate: bool,
    pub total_frames: u64,
    pub duration: Duration,
}

pub fn extract_meta_data(video_path: &str) -> Result<VideoMetaData> {
    let meta_ouput = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,avg_frame_rate,nb_frames",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path,
        ])
        .stdout(Stdio::piped())
        .output()?;

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

fn get_frame_rate(input: &str) -> Result<f64> {
    let fps_parts: Vec<&str> = input.split("/").collect();
    let fps_first: u32 = fps_parts[0].parse()?;
    let fps_last: u32 = fps_parts[1].parse()?;
    let fps: f64 = fps_first as f64 / fps_last as f64;
    let frame_rate = (fps * 100.0).round() / 100.0;

    Ok(frame_rate)
}