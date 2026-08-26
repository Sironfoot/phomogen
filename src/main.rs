pub mod app;
pub mod ui;
pub mod ffmpeg;

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use std::{io, thread};
use std::fs;

use image::{ImageFormat, ImageReader};

use ratatui::{Terminal};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::crossterm::{event, execute};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen,
    enable_raw_mode,
    LeaveAlternateScreen,
    disable_raw_mode
};

use anyhow::Result;
use sysinfo::{Disks, System};

use crate::app::{App, AppStage, ImageFile, ImageType, SystemInfo, VideoFile, VideoIndexCore, VideoIndexStatus, VideoIndexingReport};
use crate::ffmpeg::color_extractor::ColorExtractionProgress;
use crate::ui::render_ui;
use crate::ffmpeg::VideoMetadata;

const DEFAULT_MAX_CORES: u32 = 4;

fn main() -> Result<()> {
    // TODO: replace with CLI args + better error handling
    const TEST_DIR: &str = "./videos";

    let wk_dir = TEST_DIR; 

    let physical_cores = match System::physical_core_count() {
        Some(cores) => Some(cores as u32),
        None => None,
    };

    // TODO: will eventually be configurable
    let max_allowed_cores = physical_cores.unwrap_or(DEFAULT_MAX_CORES);

    let working_dir = match std::fs::canonicalize(wk_dir) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            panic!("path {wk_dir} does not exist");
        },
        Err(err) => panic!("Unknown error: {}", err),
    };
    
    let disks = Disks::new_with_refreshed_list();

    let current_disk = disks
        .list()
        .iter()
        .filter(|disk| working_dir.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count());
    
    let (total_space, free_space) = match current_disk {
        Some(disk) => (Some(disk.total_space()), Some(disk.available_space())),
        None => (None, None),
    };

    let sys_info = SystemInfo {
        available_physical_cores: physical_cores,
        max_allowed_cores: max_allowed_cores,
        total_drive_space: total_space,
        free_space: free_space,
    };

    let working_dir = working_dir.display().to_string();

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&working_dir, sys_info);
    run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;

    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()>
where
    io::Error: From<B::Error>
{
    let rc = read_video_files(&app);
    let mut images_receiver: Option<Receiver<Vec<ImageFile>>> = None;


    let mut should_render = true;

    loop {
        if should_render {
            terminal.draw(|frame| render_ui(frame, app))?;
            should_render = false;
        }

        match app.stage {
            AppStage::Initial => {
                if let Ok(videos) = rc.try_recv() {
                    app.videos = videos;

                    if app.videos.len() > 0 {
                        app.videos[0].is_selected = true;
                    }

                    app.stage = AppStage::VideoSelect;
                    should_render = true;
                }
            },
            AppStage::ImageSelect => {
                if images_receiver.is_none() {
                    images_receiver = Some(read_image_files(&app));
                }

                if let Some(rc) = &images_receiver {
                    if let Ok(images) = rc.try_recv() {
                        app.images = images;
                        should_render = true;

                        if app.images.len() > 0 {
                            if let Some(selected_image) = app.images.get_mut(0) {
                                selected_image.is_selected = true;

                                if let Ok(image) = image::open(&selected_image.full_path) {
                                    let image = if image.width() > 320 {
                                        let ratio = image.height() as f64 / image.width() as f64;
                                        let height = f64::round(320 as f64 * ratio) as u32;
                                        image.resize(320, height, ratatui_image::FilterType::Nearest)
                                    }
                                    else {
                                        image
                                    };

                                    selected_image.preview = Some(image);
                                }
                            }
                        }
                    }
                }
            },
            AppStage::GenerateMosaicDatabase => {
                let num_cores = (app.system_info.max_allowed_cores as f64 / 2.0).floor() as u32;
                let mut video_reports: Vec<VideoIndexingReport> = vec![];

                for video in app.videos.iter().filter(|v| v.is_selected) {
                    let mut video_report = VideoIndexingReport::new(&video.metadata.file_name);
                    let frames_per_core = (video.metadata.total_frames as f64 / num_cores as f64).round() as u64;

                    for core_id in 0..num_cores {
                        let core = VideoIndexCore::new(core_id, frames_per_core);
                        video_report.cores.push(core);
                    }

                    video_reports.push(video_report);
                }

                let video_report = video_reports.get_mut(0).unwrap();
                video_report.status = VideoIndexStatus::Running;

                for core in video_report.cores.as_mut_slice() {
                    core.average_fps = 123.45;
                    core.frames_processed = 12978;
                    core.status = VideoIndexStatus::Running;
                }

                app.video_indexing_report = Some(video_reports);

                should_render = true;
            },
            _ => {}
        }

        if event::poll(Duration::from_millis(250))? {
            let event = event::read()?;

            if let Event::Resize(_, _) = event {
                should_render = true;
                continue;
            }

            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') => {
                        app.stage = AppStage::Quitting;
                        break;
                    },
                    _ => {}
                }

                match app.stage {
                    AppStage::VideoSelect => {
                        match key.code {
                            KeyCode::Up => {
                                let mut video_index = app.current_video_index;

                                if video_index == 0 {
                                    video_index = app.videos.len() as u32 - 1;
                                }
                                else {
                                    video_index -= 1;
                                }

                                app.current_video_index = video_index;
                                should_render = true;
                            },
                            KeyCode::Down => {
                                let mut video_index = app.current_video_index;

                                if video_index == (app.videos.len() as u32) - 1 {
                                    video_index = 0;
                                }
                                else {
                                    video_index += 1;
                                }

                                app.current_video_index = video_index;
                                should_render = true;
                            },
                            KeyCode::Char(' ') => {
                                let video_index = app.current_video_index;

                                app.videos[video_index as usize].is_selected =
                                    !app.videos[video_index as usize].is_selected;

                                should_render = true;
                            },
                            KeyCode::Char('a') => {
                                for video in app.videos.iter_mut() {
                                    video.is_selected = true;
                                }
                            },
                            KeyCode::Enter => {
                                if app.videos.iter().any(|v| v.is_selected) {
                                    app.stage = AppStage::ImageSelect;
                                }

                                should_render = true;
                            }
                            _ => {}
                        }
                    },
                    AppStage::ImageSelect => {
                        match key.code {
                            KeyCode::Up => {
                                let mut image_index = app.current_image_index;

                                if image_index == 0 {
                                    image_index = app.images.len() as u32 - 1;
                                }
                                else {
                                    image_index -= 1;
                                }

                                app.current_image_index = image_index;
                                should_render = true;
                            },
                            KeyCode::Down => {
                                let mut image_index = app.current_image_index;

                                if image_index == (app.images.len() as u32) - 1 {
                                    image_index = 0;
                                }
                                else {
                                    image_index += 1;
                                }

                                app.current_image_index = image_index;
                                should_render = true;
                            },
                            KeyCode::Char(' ') => {
                                let image_index = app.current_image_index;

                                for image in app.images.iter_mut() {
                                    image.is_selected = false;
                                }

                                let selected_image = app.images.get_mut(image_index as usize);
                                if let Some(selected_image) = selected_image {
                                    selected_image.is_selected = true;

                                    if selected_image.preview.is_none() {
                                        let image_path = app.working_dir.join(selected_image.file_name.clone());

                                        if let Ok(image) = image::open(image_path) {
                                            let image = if image.width() > 320 {
                                                let ratio = image.height() as f64 / image.width() as f64;
                                                let height = f64::round(320 as f64 * ratio) as u32;
                                                image.resize(320, height, ratatui_image::FilterType::Nearest)
                                            }
                                            else {
                                                image
                                            };

                                            selected_image.preview = Some(image);
                                        }
                                    }
                                }

                                should_render = true;
                            },
                            KeyCode::Enter => {
                                if app.images.iter().any(|i| i.is_selected) {
                                    app.stage = AppStage::GenerateMosaicDatabase;
                                }
                                should_render = true;
                            }
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn generate_database(video: VideoMetadata, app: &App) -> Receiver<ColorExtractionProgress> {
    let (tx, rc) = mpsc::channel::<ColorExtractionProgress>();

    thread::spawn(|| {

    });

    rc 
}

fn read_video_files(app: &App) -> Receiver<Vec<VideoFile>> {
    let (tx, rc) = mpsc::channel::<Vec<VideoFile>>();
    let working_dir = app.working_dir.clone();

    thread::spawn(move || {
        const VIDEO_EXTENSIONS: &[&str] = &[
            "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv",
        ];

        let mut video_files: Vec<String> = vec![];

        let entries = fs::read_dir(&working_dir).unwrap();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap().display();
                let ext = match path.extension() {
                    Some(ext) => Some(ext.display().to_string()),
                    None => None,
                };
                
                if let Some(file_ext) = ext {
                    let allowed = VIDEO_EXTENSIONS.iter()
                        .any(|ext| ext.eq_ignore_ascii_case(&file_ext));

                    if allowed {
                        video_files.push(file_name.to_string());
                    }
                }
            }
        }

        let mut videos: Vec<VideoFile> = Vec::with_capacity(video_files.len());

        for video_file in video_files {
            let full_path = working_dir.join(video_file);
            let meta_data = VideoMetadata::extract_from(&full_path);

            if let Ok(meta_data) = meta_data {
                let video = VideoFile {
                    metadata: meta_data,
                    is_selected: false,
                };

                videos.push(video);
            }
        }

        tx.send(videos).unwrap();
    });

    rc
}

fn read_image_files(app: &App) -> Receiver<Vec<ImageFile>> {
    let (tx, rc) = mpsc::channel::<Vec<ImageFile>>();
    let working_dir = app.working_dir.clone();

    thread::spawn(move || {
        const IMAGE_EXTENSIONS: &[&str] = &[
            "jpg", "jpeg", "png", "webp", "bmp", "tiff"
        ];

        let mut image_files: Vec<String> = vec![];

        let entries = fs::read_dir(&working_dir).unwrap();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap().display();
                let ext = match path.extension() {
                    Some(ext) => Some(ext.display().to_string()),
                    None => None,
                };
                
                if let Some(file_ext) = ext {
                    let allowed = IMAGE_EXTENSIONS.iter()
                        .any(|ext| ext.eq_ignore_ascii_case(&file_ext));

                    if allowed {
                        image_files.push(file_name.to_string());
                    }
                }
            }
        }

        let mut images: Vec<ImageFile> = vec![];

        for image_file in image_files {
            let full_path = working_dir.join(image_file.clone());

            let Ok(image) = ImageReader::open(full_path.clone()) else { continue; };
            let Ok(image) = image.with_guessed_format() else { continue; };
            let Some(image_format) = image.format() else { continue; };
            let Ok((width, height)) = image.into_dimensions() else { continue; };

            let format = match image_format {
                ImageFormat::Bmp => ImageType::BMP,
                ImageFormat::Jpeg => ImageType::JPEG,
                ImageFormat::Png => ImageType::PNG,
                ImageFormat::WebP => ImageType::WEBP,
                ImageFormat::Tiff => ImageType::TIFF,
                _ => { continue; }
            };

            images.push(ImageFile {
                file_name: image_file,
                full_path: full_path,
                width: width,
                height: height,
                format: format,
                preview: None,
                is_selected: false,
            });
            
        }

        tx.send(images).unwrap();
    });

    rc
}