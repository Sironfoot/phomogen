use std::{cmp, io::Read, process::{Command, Stdio}, sync::mpsc};
use anyhow::Result;
use image::{DynamicImage, RgbImage, imageops};

use crate::ffmpeg::VideoMetadata;

const BYTES_PER_PIXEL: u32 = 3;
const DEFAULT_RESIZE_WIDTH: u32 = 1920;
const SMALLEST_RESIZED_WIDTH:u32 = 640;
const DEFAULT_MAX_FFMPEG_THREADS: u32 = 4;


#[derive(Debug, Clone)]
pub struct VideoFrameMatch {
    pub tile_index: u32,
    pub frame_index: u32,
    
    pub crop_resize: f64,
    pub crop_pos_x: f64,
    pub crop_pos_y: f64,
    pub is_flipped: bool,
}

pub struct ImageTileData {
    pub tile_index: u32,
    pub data: DynamicImage,
}

pub struct FrameExtractor {
    pub instance_id: u32,
    video: VideoMetadata,

    starting_frame_index: u32,
    ending_frame_index: u32,

    max_ffmpeg_threads: u32,
    resize_width: u32,
}

impl FrameExtractor {
    pub fn new(instance_id: u32, video: VideoMetadata, starting_frame_index: u32, ending_frame_index: u32) -> Self {
        let resize_width = cmp::min(DEFAULT_RESIZE_WIDTH, video.width);
        let max_ffmpeg_threads = DEFAULT_MAX_FFMPEG_THREADS;

        Self {
            instance_id,
            video,
            starting_frame_index,
            ending_frame_index,
            max_ffmpeg_threads: max_ffmpeg_threads,
            resize_width: resize_width,
        }
    }

    pub fn set_max_threads(&mut self, num: u32) -> &mut Self {
        self.max_ffmpeg_threads = cmp::max(1, num);
        return self;
    }

    pub fn set_resize_width(&mut self, width: u32) -> &mut Self {
        let width = cmp::max(width, SMALLEST_RESIZED_WIDTH);

        self.resize_width = cmp::min(width, self.video.width);
        return self;
    }

    pub fn run(&mut self, matched_frames: &[VideoFrameMatch], tx: mpsc::Sender<ImageTileData>, ) -> Result<()> {
        let frame_width = self.resize_width;
        let frame_height = (frame_width as f64 / self.video.aspect_ratio.ratio()).round() as u32;

        let mut frame_indices = matched_frames.iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<u32>>();

        frame_indices.sort_unstable();
        frame_indices.dedup();

        let seconds_to_target_frame = self.starting_frame_index as f64 / self.video.frame_rate;

        let mut child = Command::new("ffmpeg")
            .args([
                "-hwaccel", "auto", // TODO: need to detect GPU decode is available, fall back to CPU
                "-threads", &format!("{}", self.max_ffmpeg_threads),
                "-ss", &format!("{seconds_to_target_frame}"),
                "-i"]).arg(&self.video.full_path)
            .args([
                "-vf", &format!("scale={}:-2:flags=area", self.resize_width),

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

        let frame_size = frame_width * frame_height * BYTES_PER_PIXEL;

        let mut stdout = child.stdout.take().unwrap();
        let mut buffer = vec![0u8; frame_size as usize];

        let mut current_frame_index: u32 = self.starting_frame_index;

        loop {
            match stdout.read_exact(&mut buffer) {
                Ok(()) => {
                    if frame_indices.contains(&current_frame_index) {
                        let image = RgbImage::from_raw(
                            frame_width, frame_height, buffer.to_vec()).unwrap();

                        let matches = matched_frames.iter()
                            .filter(|frame_match| frame_match.frame_index == current_frame_index);

                        for matched_frame in matches {
                            let pos_x = f64::round((frame_width as f64 / 100.0) * matched_frame.crop_pos_x) as u32;
                            let pos_y = f64::round((frame_height as f64 / 100.0) * matched_frame.crop_pos_y) as u32;
                        
                            let cropped_width = f64::round((frame_width as f64 / 100.0) * matched_frame.crop_resize) as u32;
                            let cropped_height = f64::round((frame_height as f64 / 100.0) * matched_frame.crop_resize) as u32;
                        
                            let mut crop = imageops::crop_imm(&image, pos_x, pos_y, cropped_width, cropped_height).to_image();
                            
                            if matched_frame.is_flipped {
                                imageops::flip_horizontal_in_place(&mut crop);
                            }

                            let tile_data = ImageTileData {
                                tile_index: matched_frame.tile_index,
                                data: DynamicImage::ImageRgb8(crop),
                            };
                            
                            tx.send(tile_data).unwrap();
                        }
                    }

                    current_frame_index += 1;

                    if current_frame_index > self.ending_frame_index {
                        child.kill().unwrap();
                        break;
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                },
                Err(error) => {
                    return Err(anyhow::format_err!("Failed reading ffmpeg output: {error}"));
                }
            }
        }

        let _ = child.wait().unwrap();

        Ok(())
    }
}