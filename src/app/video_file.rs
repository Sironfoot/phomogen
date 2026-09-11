use std::{path::PathBuf, sync::Arc};

use crate::{app::{VideoIndexingReport, frame_data::VideoColorIndexDatabase}, ffmpeg::VideoMetadata};

pub struct VideoFile {
    pub metadata: VideoMetadata,
    pub is_chosen: bool,
    pub database_path: Option<PathBuf>,

    pub indexing_report: Option<VideoIndexingReport>,

    pub database: Option<Arc<VideoColorIndexDatabase>>,

    pub is_loading_database: bool,
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
            is_loading_database: false,
            total_database_frames_loaded: 0,
            total_dropped_frames: 0,
        }
    }
}