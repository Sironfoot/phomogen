use std::{ops::Add, path::{Path, PathBuf}, time::Duration};

use image::DynamicImage;

use crate::ffmpeg::{VideoMetadata, color_extractor::ColorExtractionAlgorithm};

pub struct App {
    pub stage: AppStage,

    pub system_info: SystemInfo,
    pub working_dir: PathBuf,
    pub database_dir: PathBuf,

    pub tiles_x: u32,
    pub tiles_y: u32,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,
    pub color_extraction_algorithm: ColorExtractionAlgorithm,

    pub current_image_index: u32,
    pub images: Vec<ImageFile>,
}

pub struct SystemInfo {
    pub available_physical_cores: Option<u32>,
    pub max_allowed_cores: u32,

    pub total_drive_space: Option<u64>,
    pub free_space: Option<u64>,
}

pub struct VideoFile {
    pub metadata: VideoMetadata,
    pub is_chosen: bool,
    pub database_path: Option<PathBuf>,

    pub indexing_report: Option<VideoIndexingReport>,
}

impl VideoFile {
    pub fn new(video: VideoMetadata) -> VideoFile {
        VideoFile {
            metadata: video,
            is_chosen: false,
            database_path: None,
            indexing_report: None,
        }
    }
}

pub struct ImageFile {
    pub file_name: String,
    pub full_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ImageType,
    pub preview: Option<DynamicImage>, 
    pub is_chosen: bool,
}

#[derive(Clone, Debug)]
pub struct VideoIndexingReport {
    pub file_name: String,
    pub total_frames: u64,
    pub cores: Vec<VideoIndexCore>,
    pub status: VideoIndexStatus,
}

impl VideoIndexingReport {
    pub fn new(file_name: &str, total_frames: u64) -> VideoIndexingReport {
        VideoIndexingReport {
            file_name: String::from(file_name),
            total_frames,
            cores: vec![],
            status: VideoIndexStatus::NotStarted,
        }
    }

    pub fn frames_processed(&self) -> u64 {
        self.cores.iter().map(|c| c.frames_processed).sum()
    }

    pub fn percentage_complete(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }

        let frames_processed = self.frames_processed();
        (100.0 / self.total_frames as f64) * frames_processed as f64
    }

    pub fn average_fps(&self) -> f64 {
        self.cores.iter().map(|c| c.average_fps).sum()
    }
}

#[derive(Clone, Debug)]
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
            status: VideoIndexStatus::NotStarted,
        }
    }

    pub fn percentage_complete(&self) -> f64 {
        match self.status {
            VideoIndexStatus::Finished => 100.0,
            VideoIndexStatus::Initialising | VideoIndexStatus::NotStarted => 0.0,
            VideoIndexStatus::Running => {
                (100.0 / self.total_frames as f64) * self.frames_processed as f64
            }
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum VideoIndexStatus {
    NotStarted,
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

const DATABASE_DIR: &str = "pmg_data";

impl App {
    pub fn new(wk_dir: &Path, sys_info: SystemInfo) -> App {
        let database_dir = wk_dir.join(DATABASE_DIR);

        App {
            stage: AppStage::Initial,
            system_info: sys_info,
            working_dir: PathBuf::from(wk_dir),
            database_dir: database_dir,
            tiles_x: 4,
            tiles_y: 4,
            current_video_index: 0,
            videos: vec![],
            color_extraction_algorithm: ColorExtractionAlgorithm::PixelArrayTraversal,
            current_image_index: 0,
            images: vec![],
        }
    }
}

impl App {
    pub fn total_selected_video_duration(&self) -> Duration {
        let total = self.videos.iter()
            .filter(|v| v.is_chosen)
            .map(|v| v.metadata.duration)
            .reduce(|accu, item| accu.add(item));

        match total {
            Some(total) => total,
            None => Duration::new(0, 0),
        }
    }

    pub fn total_selected_video_frames(&self) -> u64 {
        self.videos.iter()
            .filter(|v| v.is_chosen)
            .map(|v| v.metadata.total_frames)
            .sum()
    }

    pub fn total_video_indexing_progress(&self) -> f64 {
        let indexing_reports: Vec<_> = self.videos.iter()
            .filter(|v| v.indexing_report.is_some())
            .map(|v| v.indexing_report.as_ref().unwrap())
            .collect();

        let frames_processed: u64 = indexing_reports.iter()
            .map(|r| r.frames_processed())
            .sum();

        let total_frames: u64 = indexing_reports.iter()
            .map(|r| r.total_frames)
            .sum();

        if total_frames == 0 {
            return 0.0
        }

        (100.0 / total_frames as f64) * frames_processed as f64
    }
}

#[derive(PartialEq)]
pub enum AppStage {
    Initial,
    VideoSelect,
    GenerateMosaicDatabase,
    ImageSelect,
    LoadMosaicDatabase,
    Quitting,
}