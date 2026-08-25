use std::{ops::Add, time::Duration};

use image::DynamicImage;

use crate::ffmpeg::VideoMetadata;

pub struct App {
    pub stage: AppStage,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,

    pub current_image_index: u32,
    pub images: Vec<ImageFile>,
    
    pub video_indexing_report: Option<Vec<VideoIndexingReport>>,
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

pub struct VideoIndexingReport {
    pub file_name: String,
    pub cores: Vec<VideoIndexCore>,
    pub status: VideoIndexStatus,
}

impl VideoIndexingReport {
    pub fn new(file_name: &str) -> VideoIndexingReport {
        VideoIndexingReport {
            file_name: String::from(file_name),
            cores: vec![],
            status: VideoIndexStatus::Initialising,
        }
    }

    pub fn frames_processed(&self) -> u64 {
        self.cores.iter().map(|c| c.frames_processed).sum()
    }

    pub fn total_frames(&self) -> u64 {
        self.cores.iter().map(|c| c.total_frames).sum()
    }

    pub fn percentage_complete(&self) -> f64 {
        let frames_processed = self.frames_processed();
        let total_frames = self.total_frames();

        (100.0 / total_frames as f64) * frames_processed as f64
    }

    pub fn average_fps(&self) -> f64 {
        self.cores.iter().map(|c| c.average_fps).sum()
    }
}

pub struct VideoIndexCore {
    pub core_id: u32,
    pub frames_processed: u64,
    pub total_frames: u64,
    pub average_fps: f64,

    pub status: VideoIndexStatus,
}

impl VideoIndexCore {
    pub fn new(core_id: u32, total_frames: u64) -> VideoIndexCore {
        VideoIndexCore {
            core_id,
            frames_processed: 0,
            total_frames,
            average_fps: 0.0,
            status: VideoIndexStatus::Initialising,
        }
    }

    pub fn percentage_complete(&self) -> f64 {
        match self.status {
            VideoIndexStatus::Finished => 100.0,
            VideoIndexStatus::Initialising => 0.0,
            VideoIndexStatus::Running => {
                (100.0 / self.total_frames as f64) * self.frames_processed as f64
            }
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum VideoIndexStatus {
    Initialising,
    Running,
    Finished,
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
            video_indexing_report: None,
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

    pub fn total_video_indexing_progress(&self) -> f64 {
        if let Some(reports) = &self.video_indexing_report {
            let frames_processed: u64 = reports.iter().map(|r| r.frames_processed()).sum();
            let total_frames: u64 = reports.iter().map(|r| r.total_frames()).sum();

            return (100.0 / total_frames as f64) * frames_processed as f64;
        }

        0.0
    }
}

#[derive(PartialEq)]
pub enum AppStage {
    Initial,
    VideoSelect,
    ImageSelect,
    GenerateMosaicDatabase,
    Quitting,
}