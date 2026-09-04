use std::{cmp, io::Read, process::{Command, Stdio}, sync::mpsc};
use anyhow::Result;
use image::{DynamicImage, RgbImage, imageops};

use crate::ffmpeg::VideoMetadata;

const BYTES_PER_PIXEL: u32 = 3;
const DEFAULT_RESIZE_WIDTH: u32 = 1920;
const SMALLEST_RESIZED_WIDTH:u32 = 640;


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

    resize_width: u32,
}

impl FrameExtractor {
    pub fn new(instance_id: u32, video: VideoMetadata) -> Self {
        let resize_width = cmp::min(DEFAULT_RESIZE_WIDTH, video.width);

        Self {
            instance_id,
            video,
            resize_width: resize_width,
        }
    }

    pub fn set_resize_width(&mut self, width: u32) -> &mut Self {
        let width = cmp::max(width, SMALLEST_RESIZED_WIDTH);

        self.resize_width = cmp::min(width, self.video.width);
        return self;
    }

    pub fn run(&mut self, matched_frames: &[VideoFrameMatch], tx: mpsc::Sender<ImageTileData>) -> Result<()> {
        let frame_width = self.resize_width;
        let frame_height = (frame_width as f64 / self.video.aspect_ratio.ratio()).round() as u32;

        let mut frame_indices = matched_frames.iter()
            .map(|frame| frame.frame_index)
            .collect::<Vec<u32>>();

        frame_indices.sort_unstable();
        frame_indices.dedup();

        let frame_size = frame_width * frame_height * BYTES_PER_PIXEL;
        const PRE_ROLL: f64 = 1.1;

        for frame_index in frame_indices {
            let seconds_to_target_frame = frame_index as f64 / self.video.frame_rate;

            // with some advanced codecs (H.265/HEVC) seeking individual frames could potentially fall on a
            // B-frame, you can end up with a frame from 1-3 frames before or after it, rather than the exact
            // frame you want. This can lead to the incorrect frame and visual annomalies in the mosaic.
            // Explained in detail here: https://ffmpeg.org/pipermail/ffmpeg-devel/2022-February/293221.html
            // Something to do with open-GOP/CRA random-access behaviour, I guess video codecs are increadibly
            // complicated. The work around is to seek the video 1.1 seconds before the desired frame (PRE_ROLL)
            // then play forward from there until the desired frame is reached, this ensures the video
            // is decoded correctly, then the desired frame can be extracted.
            let coarse_seek = (seconds_to_target_frame - PRE_ROLL).max(0.0);
            let fine_seek = seconds_to_target_frame - coarse_seek;

            let mut child = Command::new("ffmpeg")
                .args([
                    "-hwaccel", "auto", // TODO: need to detect GPU decode is available, fall back to CPU
                    "-ss", &format!("{coarse_seek}"),
                    "-i"]).arg(&self.video.full_path)
                .args([
                    "-ss", &format!("{fine_seek}"),
                    "-frames:v", "1",
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

            let mut stdout = child.stdout.take().unwrap();
            let mut buffer = vec![0u8; frame_size as usize];

            loop {
                match stdout.read_exact(&mut buffer) {
                    Ok(()) => {
                        let image = RgbImage::from_raw(
                            frame_width, frame_height, buffer.to_vec()).unwrap();

                        let matches = matched_frames.iter()
                            .filter(|frame_match| frame_match.frame_index == frame_index);

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
        }

        Ok(())
    }
}