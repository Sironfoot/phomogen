mod app_stage;
pub mod frame_data;
mod image_file;
mod system_info;
mod terminal_theme;
mod video_file;
mod video_indexing_report;

pub use app_stage::AppStage;
pub use image_file::{ImageFile, ImageType};
pub use system_info::SystemInfo;
pub use terminal_theme::{TerminalTheme, TerminalThemeMode};
pub use video_file::VideoFile;
pub use video_indexing_report::{VideoIndexCore, VideoIndexingReport, VideoIndexStatus};

use crate::{app::frame_data::Color, ffmpeg::{color_extractor::ColorExtractionAlgorithm, crops::CropLevel}};

use std::{ops::Add, path::{Path, PathBuf}, time::{Duration, Instant}};

use terminal_colorsaurus::{color_palette, QueryOptions, ThemeMode};

const DATABASE_DIR: &str = "pmg_data";
const MOSAICS_DIR: &str = "pmg_mosaics";
const DEFAULT_COLOR_TILES: u32 = 4;

pub struct App {
    pub stage: AppStage,

    pub system_info: SystemInfo,
    pub working_dir: PathBuf,
    pub database_dir: PathBuf,
    pub mosaics_dir: PathBuf,

    pub color_tiles_x: u32,
    pub color_tiles_y: u32,

    pub mosaic_tiles_x: u32,
    pub mosaic_tiles_y: u32,

    pub current_video_index: u32,
    pub videos: Vec<VideoFile>,
    pub color_extraction_algorithm: ColorExtractionAlgorithm,

    pub current_image_index: u32,
    pub images: Vec<ImageFile>,

    pub terminal_palette: TerminalTheme,

    srgb_profile: Vec<u8>,

    timer: Instant,
    stopped_ellapsed: Option<Duration>,

    allowed_crops: Vec<CropLevel>,
}

impl App {
    pub fn new(wk_dir: &Path, sys_info: SystemInfo, srgb_profile: Vec<u8>) -> App {
        let database_dir = wk_dir.join(DATABASE_DIR);
        let mosaics_dir = wk_dir.join(MOSAICS_DIR);

        let terminal_palette = match color_palette(QueryOptions::default()) {
            Ok(palette) => {
                let mode = if palette.theme_mode() == ThemeMode::Light
                    { TerminalThemeMode::Light } else { TerminalThemeMode::Dark };

                let (fg_r, fg_g, fg_b) = palette.foreground.scale_to_8bit();
                let (bg_r, bg_g, bg_b) = palette.background.scale_to_8bit();

                TerminalTheme {
                    mode: mode,
                    foreground_color: Color { r: fg_r, g: fg_g, b: fg_b },
                    background_color: Color { r: bg_r, g: bg_g, b: bg_b },
                }
            },
            Err(_) => {
                // assume dark mode
                TerminalTheme {
                    mode: TerminalThemeMode::Dark,
                    foreground_color: Color { r: 255, g: 255, b: 255 },
                    background_color: Color { r: 0, g: 0, b: 0 },
                }
            }
        };

        App {
            stage: AppStage::Initial,
            system_info: sys_info,
            working_dir: PathBuf::from(wk_dir),
            database_dir: database_dir,
            mosaics_dir: mosaics_dir,
            color_tiles_x: DEFAULT_COLOR_TILES,
            color_tiles_y: DEFAULT_COLOR_TILES,
            mosaic_tiles_x: 40,
            mosaic_tiles_y: 40,
            current_video_index: 0,
            videos: vec![],
            color_extraction_algorithm: ColorExtractionAlgorithm::PixelArrayTraversal,
            current_image_index: 0,
            images: vec![],
            terminal_palette,
            srgb_profile: srgb_profile,
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

    pub fn get_srgb_profile(&self) -> Vec<u8> {
        self.srgb_profile.clone()
    }

    pub fn set_color_tiles(&mut self, num_x: u32, num_y: u32) {
        self.color_tiles_x = num_x;
        self.color_tiles_y = num_y;
    }

    pub fn set_mosaic_tiles(&mut self, num_x: u32, num_y: u32) {
        self.mosaic_tiles_x = num_x;
        self.mosaic_tiles_y = num_y;
    }

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