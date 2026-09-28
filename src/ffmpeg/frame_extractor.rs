use std::{cmp, io::Read, process::{Command, Stdio}};
use anyhow::Result;
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};

use crate::ffmpeg::VideoMetadata;

const BYTES_PER_PIXEL: u32 = 3;
const DEFAULT_RESIZE_WIDTH: u32 = 1920;
const SMALLEST_RESIZED_WIDTH: u32 = 640;

#[derive(Debug, Clone)]
pub struct VideoFrameMatch {
    pub tile_index: u32,
    pub frame_index: u32,
    
    pub crop_resize: f32,
    pub crop_pos_x: f32,
    pub crop_pos_y: f32,
    pub is_flipped: bool,
}

pub struct ImageTileData {
    pub tile_index: u32,
    pub data: DynamicImage,
}

pub struct FrameExtractor {
    video: VideoMetadata,
    resize_width: u32,
}

impl FrameExtractor {
    pub fn new(video: VideoMetadata) -> Self {
        let resize_width = cmp::min(DEFAULT_RESIZE_WIDTH, video.width);

        Self {
            video,
            resize_width: resize_width,
        }
    }

    pub fn set_resize_width(&mut self, width: u32) -> &mut Self {
        let width = cmp::max(width, SMALLEST_RESIZED_WIDTH);

        self.resize_width = cmp::min(width, self.video.width);
        return self;
    }

    pub fn extract(&mut self, frame_index: u32) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        let frame_width = self.resize_width;
        let frame_height = (frame_width as f64 / self.video.aspect_ratio.ratio()).round() as u32;

        let frame_size = frame_width * frame_height * BYTES_PER_PIXEL;
        const PRE_ROLL: f64 = 1.1;

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

        let mut image: Option<ImageBuffer<Rgb<u8>, Vec<u8>>> = None;

        loop {
            match stdout.read_exact(&mut buffer) {
                Ok(()) => {
                    image = RgbImage::from_raw(frame_width, frame_height, buffer.to_vec());
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

        let Some(image) = image else {
            return Err(anyhow::format_err!("Failed to retrieve image for frame"));
        };

        Ok(image)
    }
}