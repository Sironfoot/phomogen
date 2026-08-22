pub mod app;
pub mod ui;
pub mod ffmpeg;

use std::sync::mpsc;
use std::time::Duration;
use std::{io, thread};
use std::fs;

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

use crate::app::{App, AppStage, VideoFile};
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
    let (tx, rc) = mpsc::channel::<Vec<VideoFile>>();

    thread::spawn(move || {
        const VIDEO_EXTENSIONS: &[&str] = &[
            "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv",
        ];

        let mut video_files: Vec<String> = vec![];

        let entries = fs::read_dir(TEST_DIR).unwrap();

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
                    width: meta_data.width,
                    height: meta_data.height,
                    frame_rate: meta_data.frame_rate,
                    is_constant_frame_rate: !meta_data.is_variable_frame_rate,
                    total_frames: meta_data.total_frames,
                    length: meta_data.duration,
                    is_selected: false,
                };

                videos.push(video);
            }
        }

        tx.send(videos).unwrap();
    });

    loop {
        terminal.draw(|frame| render_ui(frame, app))?;

        if app.stage == AppStage::Initial {
            if let Ok(videos) = rc.try_recv() {
                app.videos = videos;

                if app.videos.len() > 0 {
                    app.videos[0].is_selected = true;
                }

                app.stage = AppStage::VideoSelect;
            }
        }

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        app.stage = AppStage::Quitting;
                        break;
                    },
                    _ => {}
                }

                if app.stage == AppStage::VideoSelect {
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
                        },
                        KeyCode::Char(' ') => {
                            let video_index = app.current_video_index;

                            app.videos[video_index as usize].is_selected =
                                !app.videos[video_index as usize].is_selected;
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
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}