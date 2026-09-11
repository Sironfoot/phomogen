pub mod app;
pub mod ui;
pub mod ffmpeg;
pub mod color_matcher;
pub mod tile_blender;
pub mod images;
pub mod tasks;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration};
use std::io;

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

use crate::app::{App, AppStage, ImageFile, SystemInfo, VideoIndexStatus, VideoIndexingReport};
use crate::color_matcher::{FrameMatch, ImageTile};
use crate::ffmpeg::color_extractor::{ColorExtractionAlgorithm};
use crate::ffmpeg::crops::CropLevel;
use crate::ui::render_ui;
use crate::images::PreviewImage;

use crate::tasks::{
    generate_database,
    read_video_files,
    read_image_files,
    load_databases,
    load_databases::LoadDatabaseProgressReport,
    calculate_image_colors,
    find_matches,
    generate_mosaic,
    generate_mosaic::MosaicGenerationReport
};

fn main() -> Result<()> {
    // TODO: replace with CLI args + better error handling
    const TEST_DIR: &str = "./videos";
    const TEST_COLOR_TILES: u32 = 8;
    const DISABLE_AGGRESIVE_CROPS: bool = false;
    const DISABLE_ALL_CROPS: bool = false;

    let wk_dir = TEST_DIR;
    let num_color_tiles = TEST_COLOR_TILES;
    
    let working_dir = match std::fs::canonicalize(wk_dir) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            panic!("path {wk_dir} does not exist");
        },
        Err(err) => panic!("Unknown error: {}", err),
    };

    let sys_info = SystemInfo::init(&working_dir)?;

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let srgb_profile = include_bytes!("../assets/sRGB2014.icc");

    let mut app = App::new(&working_dir, sys_info, srgb_profile.to_vec());
    app.set_mosaic_tiles(40, 40);
    app.set_color_tiles(num_color_tiles, num_color_tiles);
    app.color_extraction_algorithm = ColorExtractionAlgorithm::SummedAreaTable;

    if DISABLE_AGGRESIVE_CROPS {
        app.disallow_crop_level(CropLevel::Aggressive);
    }

    if DISABLE_ALL_CROPS {
        app.disallow_crop_level(CropLevel::Aggressive);
        app.disallow_crop_level(CropLevel::Moderate);
    }

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
    let rc = read_video_files::run(&app);
    let mut images_receiver: Option<Receiver<Vec<ImageFile>>> = None;
    let mut color_extractor_receiver: Option<Receiver<VideoIndexingReport>> = None;
    let mut load_database_receiver: Option<Receiver<LoadDatabaseProgressReport>> = None;
    let mut calculate_image_colors_receiver: Option<Receiver<Vec<ImageTile>>> = None;
    let mut find_matches_receiver: Option<Receiver<FrameMatch>> = None;
    let mut generate_mosaic_receiver: Option<Receiver<MosaicGenerationReport>> = None;

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
                        app.videos[0].is_chosen = true;
                    }

                    app.stage = AppStage::VideoSelect;
                    should_render = true;
                }
            },
            AppStage::VideoSelect => {
                // TODO: hot reloading of video list
            },
            AppStage::GenerateMosaicDatabase => {
                if color_extractor_receiver.is_none() {
                    let next_video = app.videos.iter().find(|v| v.is_chosen && v.database_path.is_none());

                    match next_video {
                        Some(video) => {
                            color_extractor_receiver = Some(generate_database::run(&video.metadata, &app))
                        },
                        None => {
                            app.stage = AppStage::LoadMosaicDatabase;
                            should_render = true;
                        }
                    }
                }

                if let Some(rc) = &color_extractor_receiver {
                    let reports: Vec<VideoIndexingReport> = rc.try_iter().collect();

                    for report in reports {
                        let video = app.videos.iter_mut()
                            .find(|v| v.metadata.file_name == report.file_name);

                        if let Some(video) = video {
                            let is_finished = report.status == VideoIndexStatus::Finished;
                            video.indexing_report = Some(report);

                            if is_finished {
                                let database_file_name = format!("{}-{}x{}.pmgd", video.metadata.file_name, app.color_tiles_x, app.color_tiles_y);
                                video.database_path = Some(app.database_dir.join(&database_file_name));

                                color_extractor_receiver = None;
                            }
                        }

                        should_render = true;
                    }
                }
            },
            AppStage::LoadMosaicDatabase => {
                if load_database_receiver.is_none() {
                    let requires_loading = app.videos.iter().any(|v| {
                        v.is_chosen &&
                        v.database_path.is_some() &&
                        v.database.is_none()
                    });

                    if requires_loading {
                        load_database_receiver = Some(load_databases::run(&app));
                    }
                    else {
                        app.stage = AppStage::ImageSelect;
                        should_render = true;
                    }
                }

                if let Some(rc) = &load_database_receiver {
                    let reports: Vec<LoadDatabaseProgressReport> = rc.try_iter().collect();

                    for report in reports {
                        let video = app.videos.iter_mut()
                            .find(|v| v.metadata.file_name == report.video_file_name);

                        if let Some(video) = video {
                            video.total_database_frames_loaded = report.total_frames_processed;
                            video.total_dropped_frames = report.dropped_frames;
                            video.is_loading_database = true;
                            
                            if let Some(database) = report.database {
                                video.database = Some(Arc::new(database));
                                video.is_loading_database = false;

                                let more_videos = app.videos.iter().any(|v| {
                                    v.is_chosen &&
                                    v.database_path.is_some() &&
                                    v.database.is_none()
                                });

                                if !more_videos {
                                    app.stage = AppStage::ImageSelect;
                                    load_database_receiver = None;
                                }
                            }

                            should_render = true;
                        }
                    }
                }
            },
            AppStage::ImageSelect => {
                if images_receiver.is_none() {
                    images_receiver = Some(read_image_files::run(&app));
                }

                if let Some(rc) = &images_receiver {
                    if let Ok(images) = rc.try_recv() {
                        app.images = images;
                        should_render = true;

                        if app.images.len() > 0 {
                            if let Some(selected_image) = app.images.get_mut(0) {
                                selected_image.is_chosen = true;
                                selected_image.preview = PreviewImage::new(&selected_image.full_path).ok();
                            }
                        }
                    }
                }
            },
            AppStage::ProcessImage => {
                if calculate_image_colors_receiver.is_none() {
                    calculate_image_colors_receiver = Some(calculate_image_colors::run(&app));
                }

                if let Some(rc) = &calculate_image_colors_receiver {
                    if let Ok(image_tiles) = rc.try_recv() {
                        if let Some(image) = app.images.iter_mut().find(|i| i.is_chosen) {
                            image.image_tiles = Some(Arc::new(image_tiles));
                            
                            if let Some(preview_image) = image.preview.as_mut() {
                                preview_image.generate_progress_image(app.mosaic_tiles_x, app.mosaic_tiles_y);
                            }

                            calculate_image_colors_receiver = None;

                            app.stage = AppStage::FindingMatches;
                            should_render = true;
                        }
                    }
                }
            },
            AppStage::FindingMatches => {
                if find_matches_receiver.is_none() {
                    app.reset_timer();
                    find_matches_receiver = Some(find_matches::run(app));
                }

                if let Some(rc) = &find_matches_receiver {
                    let matches: Vec<FrameMatch> = rc.try_iter().collect();

                    if matches.len() > 0 {
                        let chosen_image = app.images.iter_mut()
                            .find(|i| i.is_chosen && i.image_tiles.is_some());

                        if let Some(chosen_image) = chosen_image {
                            if chosen_image.matched_tiles.is_none() {
                                chosen_image.matched_tiles = Some(vec![]);
                            }

                            for frame_match in matches {
                                let tile_index = frame_match.tile_index;
                                chosen_image.matched_tiles.as_mut().unwrap().push(frame_match);

                                let row = tile_index / app.mosaic_tiles_x;
                                let col = tile_index % app.mosaic_tiles_y;

                                if let Some(preview_image) = chosen_image.preview.as_mut() {
                                    preview_image.add_progress_tile(col, row);
                                }
                            }

                            let total_mosaic_tiles = (app.mosaic_tiles_x * app.mosaic_tiles_y) as usize;
                            let completed_mosaic_tiles = chosen_image.matched_tiles.as_ref().unwrap().len();

                            let is_finished = completed_mosaic_tiles == total_mosaic_tiles;
                            if is_finished {
                                app.stop_timer();
                                app.stage = AppStage::GeneratingMosaic;
                            }
                        }

                        should_render = true;
                    }
                }
            },
            AppStage::GeneratingMosaic => {
                if generate_mosaic_receiver.is_none() {
                    generate_mosaic_receiver = Some(generate_mosaic::run(app).expect("Error"));
                }

                if let Some(rc) = &generate_mosaic_receiver {
                    let reports: Vec<MosaicGenerationReport> = rc.try_iter().collect();

                    if reports.len() > 0 {
                        for _ in reports {
                            
                        }
                    }
                }
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

                                app.videos[video_index as usize].is_chosen =
                                    !app.videos[video_index as usize].is_chosen;

                                should_render = true;
                            },
                            KeyCode::Char('a') => {
                                for video in app.videos.iter_mut() {
                                    video.is_chosen = true;
                                }
                                should_render = true;
                            },
                            KeyCode::Enter => {
                                let at_least_one_selected= app.videos.iter()
                                    .any(|v| v.is_chosen);

                                if at_least_one_selected {
                                    let require_database = app.videos.iter_mut()
                                        .filter(|v| v.is_chosen && v.database_path.is_none())
                                        .collect::<Vec<_>>();

                                    if require_database.len() > 0 {
                                        for video in require_database {
                                            let report = VideoIndexingReport::new(&video.metadata.file_name, video.metadata.total_frames);
                                            video.indexing_report = Some(report);
                                        }

                                        app.stage = AppStage::GenerateMosaicDatabase;
                                    }
                                    else {
                                        let require_loading = app.videos.iter_mut()
                                            .filter(|v| v.is_chosen && v.database_path.is_some() && v.database.is_none())
                                            .collect::<Vec<_>>();

                                        if require_loading.len() > 0 {
                                            for video in require_loading {
                                                video.is_loading_database = true;
                                            }

                                            app.stage = AppStage::LoadMosaicDatabase;
                                        }
                                        else {
                                            app.stage = AppStage::ImageSelect
                                        }
                                    }
                                    
                                    should_render = true;
                                }
                            },
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
                                    image.is_chosen = false;
                                }

                                let selected_image = app.images.get_mut(image_index as usize);
                                if let Some(selected_image) = selected_image {
                                    selected_image.is_chosen = true;
                                    if selected_image.preview.is_none() {
                                        selected_image.preview = PreviewImage::new(&selected_image.full_path).ok();
                                    }
                                }

                                should_render = true;
                            },
                            KeyCode::Enter => {
                                if app.images.iter().any(|i| i.is_chosen) {
                                    app.stage = AppStage::ProcessImage;
                                }
                                should_render = true;
                            },
                            KeyCode::Backspace => {
                                app.stage = AppStage::VideoSelect;
                                should_render = true;
                            },
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