use std::{io::{BufReader, BufRead}, process::{Command, Stdio}};

use anyhow::Result;

fn main() -> Result<()> {
    let video_path = "videos/mustangs-variable.mp4";
    let output_video = "videos/mustangs-variable-fixed.mp4";

    let mut child = Command::new("ffmpeg")
        .args([
            "-y", "-i", video_path, // -y auto overwrite output file
            "-vf", "fps=30,scale=1920:-2",
            "-c:v", "h264_videotoolbox", // Apple hardware encoder (see table below)
            "-b:v", "30M", // 30Mb/s
            "-c:a", "copy",
            "-progress", "pipe:1",
            "-nostats",
            "-loglevel", "error",
            output_video,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;


    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    /*
        frame=40
        fps=0.00
        stream_0_0_q=-0.0
        bitrate=5050.8kbits/s
        total_size=2359344
        out_time_us=3736961
        out_time_ms=3736961
        out_time=00:00:03.736961
        dup_frames=0
        drop_frames=0
        speed=7.43x
        progress=continue
     */

    for line in reader.lines() {
        let line = line?;

        if let Some((key, value)) = line.split_once('=') {
            match key {
                "frame" => {
                    println!("Frame: {value}");
                }
                "fps" => {
                    println!("FPS: {value}");
                }
                "out_time" => {
                    println!("Time: {value}");
                }
                "speed" => {
                    println!("Speed: {value}");
                }
                "progress" if value == "end" => {
                    println!("Finished");
                }
                _ => {}
            }
        }
    }

    /*
        | Platform/GPU  | Hardware decode | H.264 encode        | HEVC encode         |
        |---------------|-----------------|---------------------|---------------------|
        | Apple Silicon | videotoolbox    | h264_videotoolbox   | hevc_videotoolbox   |
        | NVIDIA        | cuda / NVDEC    | h264_nvenc          | hevc_nvenc          |
        | AMD Windows.  | d3d11va         | h264_amf            | hevc_amf            |
        | Intel Windows | qsv             | h264_qsv            | hevc_qsv            |


        -c:v libx264 -crf 18 -preset medium <-- software (CPU) setting
    */

    let status = child.wait()?;

    if !status.success() {
        panic!("ffmpeg failed!");
    }

    Ok(())
}