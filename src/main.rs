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

use crate::app::{App, AppStage, ImageFile, ImageType, VideoFile};
use crate::ui::render_ui;
use crate::ffmpeg::VideoMetadata;

const TEST_DIR: &str = "./videos";

fn main() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
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
    let rc = read_video_files(TEST_DIR);
    let mut images_receiver: Option<Receiver<Vec<ImageFile>>> = None;

    let mut should_render = true;

    loop {
        if should_render {
            terminal.draw(|frame| render_ui(frame, app))?;
            should_render = false;
        }

        //println!("{}", terminal.size().unwrap().width);

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
                    images_receiver = Some(read_image_files(TEST_DIR));
                }

                if let Some(rc) = &images_receiver {
                    if let Ok(images) = rc.try_recv() {
                        app.images = images;
                        should_render = true;

                        if app.images.len() > 0 {
                            if let Some(selected_image) = app.images.get_mut(0) {
                                selected_image.is_selected = true;

                                let image_path = format!("{TEST_DIR}/{}", selected_image.file_name);

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
                    }
                }
            }
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
                                        let image_path = format!("{TEST_DIR}/{}", selected_image.file_name);

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
                                    app.stage = AppStage::BeginProcessing;
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

fn read_video_files(dir: &str) -> Receiver<Vec<VideoFile>> {
    let (tx, rc) = mpsc::channel::<Vec<VideoFile>>();
    let dir = String::from(dir);

    thread::spawn(move || {
        const VIDEO_EXTENSIONS: &[&str] = &[
            "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv",
        ];

        let mut video_files: Vec<String> = vec![];

        let entries = fs::read_dir(dir).unwrap();

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
            let full_path = format!("{TEST_DIR}/{video_file}");
            let meta_data = VideoMetadata::extract_from(&full_path);

            if let Ok(meta_data) = meta_data {
                let video = VideoFile {
                    file_name: video_file,
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

fn read_image_files(dir: &str) -> Receiver<Vec<ImageFile>> {
    let (tx, rc) = mpsc::channel::<Vec<ImageFile>>();
    let dir = String::from(dir);

    thread::spawn(move || {
        const IMAGE_EXTENSIONS: &[&str] = &[
            "jpg", "jpeg", "png", "webp", "bmp", "tiff"
        ];

        let mut image_files: Vec<String> = vec![];

        let entries = fs::read_dir(dir).unwrap();

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
            let full_path = format!("{TEST_DIR}/{image_file}");

            let Ok(image) = ImageReader::open(full_path) else { continue; };
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
                file_name:
                image_file,
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