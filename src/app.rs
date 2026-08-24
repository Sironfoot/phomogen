use std::{ops::Add, time::Duration};

use image::DynamicImage;

use crate::ffmpeg::VideoMetadata;

pub struct App {
    pub stage: AppStage,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,

    pub current_image_index: u32,
    pub images: Vec<ImageFile>,
}

pub struct VideoFile {
    pub file_name: String,
    pub metadata: VideoMetadata,
    pub is_selected: bool,
}

pub struct ImageFile {
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub format: ImageType,
    pub preview: Option<DynamicImage>, 
    pub is_selected: bool,
}

pub enum ImageType {
    BMP,
    JPEG,
    PNG,
    WEBP,
    TIFF,
}

impl App {
    pub fn new() -> App {
        App {
            stage: AppStage::Initial,
            current_video_index: 0,
            videos: vec![],
            current_image_index: 0,
            images: vec![],
        }
    }
}

impl App {
    pub fn total_selected_video_duration(&self) -> Duration {
        let total = self.videos.iter()
            .filter(|v| v.is_selected)
            .map(|v| v.metadata.duration)
            .reduce(|accu, item| accu.add(item));

        match total {
            Some(total) => total,
            None => Duration::new(0, 0),
        }
    }

    pub fn total_selected_video_frames(&self) -> u64 {
        self.videos.iter()
            .filter(|v| v.is_selected)
            .map(|v| v.metadata.total_frames)
            .sum()
    }
}

#[derive(PartialEq)]
pub enum AppStage {
    Initial,
    VideoSelect,
    ImageSelect,
    BeginProcessing,
    Quitting,
}