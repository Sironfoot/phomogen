use std::{ops::Add, time::Duration};

pub struct App {
    pub stage: AppStage,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,

    pub images: Vec<ImageFile>,
}

pub struct VideoFile {
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub is_constant_frame_rate: bool,

    pub length: Duration,
    pub total_frames: u64,

    pub is_selected: bool,
}

pub struct ImageFile {
    pub file_name: String,
    pub width: u32,
    pub height: u32,
}

impl App {
    pub fn new() -> App {
        App {
            stage: AppStage::Initial,
            current_video_index: 0,
            videos: vec![],
            images: vec![],
        }
    }
}

impl App {
    pub fn total_selected_video_duration(&self) -> Duration {
        let total = self.videos.iter()
            .filter(|v| v.is_selected)
            .map(|v| v.length)
            .reduce(|accu, item| accu.add(item));

        match total {
            Some(total) => total,
            None => Duration::new(0, 0),
        }
    }

    pub fn total_selected_video_frames(&self) -> u64 {
        self.videos.iter()
            .filter(|v| v.is_selected)
            .map(|v| v.total_frames)
            .sum()
    }
}

#[derive(PartialEq)]
pub enum AppStage {
    Initial,
    VideoSelect,
    ImageSelect,
    Quitting,
}