mod system_info;
pub use system_info::SystemInfo;

pub mod frame_data;

use std::{ops::Add, path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

use image::DynamicImage;

use crate::{app::frame_data::Color, color_matcher::FrameMatch, ffmpeg::{VideoMetadata, color_extractor::ColorExtractionAlgorithm, crops::CropLevel}};
use crate::app::frame_data::VideoColorIndexDatabase;

pub struct App {
    pub stage: AppStage,

    pub system_info: SystemInfo,
    pub working_dir: PathBuf,
    pub database_dir: PathBuf,

    pub color_tiles_x: u32,
    pub color_tiles_y: u32,

    pub mosaic_tiles_x: u32,
    pub mosaic_tiles_y: u32,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,
    pub color_extraction_algorithm: ColorExtractionAlgorithm,

    pub current_image_index: u32,
    pub images: Vec<ImageFile>,

    timer: Instant,
    stopped_ellapsed: Option<Duration>,

    allowed_crops: Vec<CropLevel>,
}

pub struct VideoFile {
    pub metadata: VideoMetadata,
    pub is_chosen: bool,
    pub database_path: Option<PathBuf>,

    pub indexing_report: Option<VideoIndexingReport>,

    pub database: Option<Arc<VideoColorIndexDatabase>>,
    pub total_database_frames_loaded: u32,
    pub total_dropped_frames: u32,
}

impl VideoFile {
    pub fn new(video: VideoMetadata) -> VideoFile {
        VideoFile {
            metadata: video,
            is_chosen: false,
            database_path: None,
            indexing_report: None,
            database: None,
            total_database_frames_loaded: 0,
            total_dropped_frames: 0,
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

    pub image_tiles: Option<Arc<Vec<ImageTile>>>,

    pub matched_tiles: Option<Vec<FrameMatch>>,
}

impl ImageFile {
    pub fn new(file_name: &str, full_path: &Path, width: u32, height: u32, format: ImageType) -> ImageFile {
        ImageFile {
            file_name: String::from(file_name),
            full_path: PathBuf::from(full_path),
            width,
            height,
            format,
            preview: None,
            is_chosen: false,
            image_tiles: None,
            matched_tiles: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageTile {
    pub colors: Vec<Color>,
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

    pub fn total_memory_usage(&self) -> u64 {
        self.cores.iter().map(|c| c.memory_usage).sum()
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
    pub instance_id: u32,
    pub frames_processed: u64,
    pub total_frames: u64,
    pub average_fps: f64,
    pub memory_usage: u64,

    pub status: VideoIndexStatus,
}

impl VideoIndexCore {
    pub fn new(instance_id: u32, total_frames: u64) -> VideoIndexCore {
        VideoIndexCore {
            instance_id,
            frames_processed: 0,
            total_frames,
            average_fps: 0.0,
            memory_usage: 0,
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
const DEFAULT_COLOR_TILES: u32 = 4;

impl App {
    pub fn new(wk_dir: &Path, sys_info: SystemInfo) -> App {
        let database_dir = wk_dir.join(DATABASE_DIR);

        App {
            stage: AppStage::Initial,
            system_info: sys_info,
            working_dir: PathBuf::from(wk_dir),
            database_dir: database_dir,
            color_tiles_x: DEFAULT_COLOR_TILES,
            color_tiles_y: DEFAULT_COLOR_TILES,
            mosaic_tiles_x: 40,
            mosaic_tiles_y: 40,
            current_video_index: 0,
            videos: vec![],
            color_extraction_algorithm: ColorExtractionAlgorithm::PixelArrayTraversal,
            current_image_index: 0,
            images: vec![],
            timer: Instant::now(),
            stopped_ellapsed: None,
            allowed_crops: vec![CropLevel::Essential, CropLevel::Moderate, CropLevel::Aggressive],
        }
    }

    pub fn reset_timer(&mut self) {
        self.timer = Instant::now();
        self.stopped_ellapsed = None;
    }

    pub fn stop_timer(&mut self) {
        self.stopped_ellapsed = Some(self.timer.elapsed());
    }

    pub fn timer_ellapsed(&self) -> Duration {
        if let Some(stopped_timer) = self.stopped_ellapsed {
            return stopped_timer;
        }

        self.timer.elapsed()
    }

    pub fn disallow_crop_level(&mut self, crop_level: CropLevel) {
        if crop_level != CropLevel::Essential {
            if let Some(position) = self.allowed_crops.iter().position(|c| c == &crop_level) {
                self.allowed_crops.remove(position);
            }
        }
    }

    pub fn allow_crop_level(&mut self, crop_level: CropLevel) {
        if !self.allowed_crops.contains(&crop_level) {
            self.allowed_crops.push(crop_level);
        }
    }

    pub fn allowed_crops(&self) -> &[CropLevel] {
        self.allowed_crops.iter().as_slice()
    }

    pub fn set_color_tiles(&mut self, num_x: u32, num_y: u32) {
        self.color_tiles_x = num_x;
        self.color_tiles_y = num_y;
    }

    pub fn set_mosaic_tiles(&mut self, num_x: u32, num_y: u32) {
        self.mosaic_tiles_x = num_x;
        self.mosaic_tiles_y = num_y;
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
    LoadMosaicDatabase,
    ImageSelect,
    ProcessImage,
    FindingMatches,
    GeneratingMosaic,
    Quitting,
}