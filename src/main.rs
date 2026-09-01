pub mod app;
pub mod ui;
pub mod ffmpeg;
pub mod color_matcher;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration};
use std::{cmp, io, thread};
use std::io::BufRead;
use std::fs::{self, File};

use image::imageops::FilterType;
use image::{GenericImage, GenericImageView, ImageBuffer, ImageFormat, ImageReader, Rgb, RgbImage, imageops};

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

use crate::app::{App, AppStage, ImageFile, ImageTile, ImageType, SystemInfo, VideoFile, VideoIndexCore, VideoIndexStatus, VideoIndexingReport};
use crate::color_matcher::{ColorMatcher, FrameMatch};
use crate::ffmpeg::color_extractor::{ColorExtractionAlgorithm, ColorExtractionProgress, ColorExtractor};
use crate::ffmpeg::frame_extractor::{FrameExtractor, ImageTileData, VideoFrameMatch};
use crate::ui::render_ui;
use crate::ffmpeg::VideoMetadata;
use crate::app::frame_data::{Color, FrameCrop, FrameData, VideoColorIndexDatabase};

fn main() -> Result<()> {
    // TODO: replace with CLI args + better error handling
    const TEST_DIR: &str = "./videos";

    let wk_dir = TEST_DIR;
    
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

    let mut app = App::new(&working_dir, sys_info);
    app.set_mosaic_tiles(10, 10);

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
                            color_extractor_receiver = Some(generate_database(&video.metadata, &app))
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
                                let database_file_name = format!("{}.pmgd", video.metadata.file_name);
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
                    let next_video = app.videos.iter()
                        .find(|v| {
                            v.is_chosen &&
                            v.database_path.is_some() &&
                            v.database.is_none()
                        });

                    match next_video {
                        Some(video) => {
                            load_database_receiver = Some(load_database(&video, &app))
                        },
                        None => {
                            app.stage = AppStage::ImageSelect;
                            should_render = true;
                        }
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
                            
                            if let Some(database) = report.database {
                                video.database = Some(Arc::new(database));
                                load_database_receiver = None;
                            }

                            should_render = true;
                        }
                    }
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
                                selected_image.is_chosen = true;

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
            AppStage::ProcessImage => {
                if calculate_image_colors_receiver.is_none() {
                    calculate_image_colors_receiver = Some(calculate_image_colors(&app));
                }

                if let Some(rc) = &calculate_image_colors_receiver {
                    if let Ok(image_tiles) = rc.try_recv() {
                        if let Some(image) = app.images.iter_mut().find(|i| i.is_chosen) {
                            image.image_tiles = Some(Arc::new(image_tiles));
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
                    find_matches_receiver = Some(find_matches(app));
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
                                chosen_image.matched_tiles.as_mut().unwrap().push(frame_match);
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
                    generate_mosaic_receiver = Some(generate_mosaic(app).expect("Error"));
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
                            },
                            KeyCode::Enter => {
                                let chosen_videos: Vec<&VideoFile> = app.videos.iter()
                                    .filter(|v| v.is_chosen)
                                    .collect();

                                if chosen_videos.len() > 0 {
                                    let require_database: Vec<_> = app.videos.iter_mut()
                                        .filter(|v| v.is_chosen && v.database_path.is_none())
                                        .collect();

                                    if require_database.len() > 0 {
                                        for video in require_database {
                                            let report = VideoIndexingReport::new(&video.metadata.file_name, video.metadata.total_frames);
                                            video.indexing_report = Some(report);
                                        }

                                        app.stage = AppStage::GenerateMosaicDatabase;
                                    }
                                    else {
                                        app.stage = AppStage::LoadMosaicDatabase;
                                    }
                                    
                                    should_render = true;
                                }
                            }
                            _ => {}
                        }
                    },
                    AppStage::LoadMosaicDatabase => {
                        match key.code {
                            KeyCode::Enter => {
                                app.stage = AppStage::ImageSelect;
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
                                    image.is_chosen = false;
                                }

                                let selected_image = app.images.get_mut(image_index as usize);
                                if let Some(selected_image) = selected_image {
                                    selected_image.is_chosen = true;

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
                                if app.images.iter().any(|i| i.is_chosen) {
                                    app.stage = AppStage::ProcessImage;
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

fn generate_database(video: &VideoMetadata, app: &App) -> Receiver<VideoIndexingReport> {
    let (video_progress_sender, video_progress_receiver) =
        mpsc::channel::<VideoIndexingReport>();

    let database_dir = app.database_dir.clone();

    let max_allowed_cores = app.system_info.max_allowed_cores();

    let tiles_x = app.tiles_x;
    let tiles_y = app.tiles_y;

    let video = video.clone();

    thread::spawn(move || {
        // create the database folder
        let database_dir_exists = fs::exists(&database_dir).unwrap_or(false);
        if !database_dir_exists {
            fs::create_dir(&database_dir).unwrap();
        }

        let mut report = VideoIndexingReport::new(&video.file_name, video.total_frames);
        report.status = VideoIndexStatus::Initialising;

        // number of FFMPEG workers is half number of CPU cores with
        // each FFMPEG instance using 2 cores eeach
        let num_workers = (max_allowed_cores as f64 / 1.0).floor() as usize;
        let ffmpeg_threads: u32 = 1;

        let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(num_workers);
        let (tx, rc) = mpsc::channel::<ColorExtractionProgress>();

        // 10 frames / 3 threads: 10 / 3 floored = 3
        let frames_per_worker = f64::floor(video.total_frames as f64 / num_workers as f64) as u64;
        // remainder on division 10 / 3 = 1
        let remaining_frames = video.total_frames % num_workers as u64;
    
        for worker_index in 0..num_workers {
            let is_last = worker_index == (num_workers - 1);

            let starting_frame_index = worker_index as u64 * frames_per_worker;

            //  10 frames / 3 threads, thread 1 = 1,2,3, thread 2 = 4,5,6, thread 3 = 6,7,8,10
            let ending_frame_index = match is_last {
                true => (starting_frame_index + frames_per_worker) + remaining_frames,
                false => starting_frame_index + frames_per_worker
            };

            let total_frames_for_this_worker = match is_last {
                true => frames_per_worker + remaining_frames,
                false => frames_per_worker,
            };

            let temp_file_name = format!("{}_core-{worker_index}_temp.pmgd", video.file_name);
            let temp_file_path = database_dir.join(temp_file_name);

            let video = video.clone();
            let tx = tx.clone();

            let mut worker_report = VideoIndexCore::new(
                worker_index as u32,
                total_frames_for_this_worker);
            worker_report.status = VideoIndexStatus::Initialising;

            report.cores.push(worker_report);

            workers.push(thread::spawn(move || {
                let mut extractor = ColorExtractor::init(
                    worker_index as u32,
                    video,
                    starting_frame_index,
                    ending_frame_index,
                    tiles_x,
                    tiles_y,
                    temp_file_path.as_path()).unwrap();

                extractor.set_algorithm(ColorExtractionAlgorithm::PixelArrayTraversal);
                extractor.set_resize_width(1920);
                extractor.set_max_threads(ffmpeg_threads);

                extractor.run(tx).unwrap();
            }));
        }

        drop(tx);

        video_progress_sender.send(report.clone()).unwrap();

        for extraction_progress in rc {
            let mut inner_report = report;
            inner_report.status = VideoIndexStatus::Running;

            if let Some(core) = inner_report.cores.iter_mut()
                .find(|c| c.instance_id == extraction_progress.instance_id) {
                
                core.frames_processed = extraction_progress.total_frames_processed;
                core.average_fps = extraction_progress.average_fps;
                core.memory_usage = extraction_progress.memory_usage;

                core.status = match core.percentage_complete() {
                    100.0 => VideoIndexStatus::Finished,
                    _ => VideoIndexStatus::Running,
                };
            }

            let finished = inner_report.cores.iter()
                .all(|c| c.status == VideoIndexStatus::Finished);

            if finished {
                inner_report.status = VideoIndexStatus::Finished;
            }

            video_progress_sender.send(inner_report.clone()).unwrap();

            report = inner_report;
        }

        // wait for all FFMPEG instances to finish
        for worker in workers {
            worker.join().unwrap();
        }

        // join all the temp files together into something like:
        // wk_dir/pmg_data/my_holiday.mp4.pmgd
        let data_file_name = format!("{}.pmgd", video.file_name);
        let full_data_file_path = database_dir.join(data_file_name);
        let mut data_file = File::create(full_data_file_path).unwrap();

        for worker_index in 0..num_workers {
            let temp_file_name = format!("{}_core-{worker_index}_temp.pmgd", video.file_name);
            let temp_file_path = database_dir.join(temp_file_name);
            let mut temp_file = File::open(&temp_file_path).unwrap();

            std::io::copy(&mut temp_file, &mut data_file).unwrap();
            std::fs::remove_file(&temp_file_path).unwrap();
        }
    });

    video_progress_receiver 
}

fn read_video_files(app: &App) -> Receiver<Vec<VideoFile>> {
    let (tx, rc) = mpsc::channel::<Vec<VideoFile>>();
    let working_dir = app.working_dir.clone();
    let database_dir = app.database_dir.clone();

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
            let full_path = working_dir.join(&video_file);
            let meta_data = VideoMetadata::extract_from(&full_path);

            let data_file = format!("{video_file}.pmgd");
            let full_data_path = database_dir.join(&data_file);

            let data_exists = fs::exists(&full_data_path).unwrap_or(false);

            if let Ok(meta_data) = meta_data {
                let mut video = VideoFile::new(meta_data);
                video.database_path = if data_exists { Some(full_data_path) } else { None };

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

            images.push(ImageFile::new(
                image_file.as_str(),
                full_path.as_path(),
                width,
                height,
                format,
            ));
            
        }

        tx.send(images).unwrap();
    });

    rc
}


struct LoadDatabaseProgressReport {
    video_file_name: String,
    total_frames_processed: u32,
    dropped_frames: u32,
    database: Option<VideoColorIndexDatabase>,
}

fn load_database(video: &VideoFile, app: &App) -> Receiver<LoadDatabaseProgressReport> {
    let (tx, rc) = mpsc::channel::<LoadDatabaseProgressReport>();

    let tiles_x = app.tiles_x;
    let tiles_y = app.tiles_y;
    let total_colors = (tiles_x * tiles_y) as usize;

    let video_file_name = video.metadata.file_name.clone();
    let total_frames = video.metadata.total_frames as u32;
    let database_path = video.database_path.clone().unwrap();

    thread::spawn(move || {
        const REPORT_PROGRESS_AFTER_FRAMES: u32 = 1234;
        let mut total_frames_added: u32 = 0;

        let mut frames: HashMap<u32, FrameData> = HashMap::with_capacity(total_frames as usize);

        let data_file = File::open(&database_path).unwrap();
        let mut reader = io::BufReader::with_capacity(256 * 1024, data_file);

        let mut dropped_frames: u32 = 0;

        let max_line_length = 222;
        let mut line = String::with_capacity(max_line_length);
        
        // e.g. 0 100 0 0 108,105,100 99,96,99 85,85,77....
        loop {
            line.clear();

            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {},
                Err(_) => break,
            }

            let mut parts = line.split_ascii_whitespace();

            // since the database is a basic text file, we needto deal with the potential
            // that it's been opened and tampered with

            // get frame index
            let Some(frame_index) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
                dropped_frames += 1;
                continue;
            };

            // check frame index is valid, can't be more than frames in the video
            if (frame_index + 1) > total_frames {
                dropped_frames += 1;
                continue;
            }

            // get resize precentage
            let Some(resize_percentage) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
                dropped_frames += 1;
                continue;
            };

            // should be in range 1 - 100
            if resize_percentage < 1.0 || resize_percentage > 100.0 {
                dropped_frames += 1;
                continue;
            }

            // get pos X
            let Some(pos_x_percentage) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
                dropped_frames += 1;
                continue;
            };

            if pos_x_percentage > 100.0 {
                dropped_frames += 1;
                continue;
            }

            // get pos Y
            let Some(pos_y_percentage) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
                dropped_frames += 1;
                continue;
            };

            if pos_y_percentage > 100.0 {
                dropped_frames += 1;
                continue;
            }

            let mut colors: Vec<Color> = Vec::with_capacity(total_colors);

            while let Some(color) = parts.next() {
                let mut rgb= color.split(',');

                let Some(r) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                    continue;
                };

                let Some(g) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                    continue;
                };

                let Some(b) = rgb.next().and_then(|c| c.parse::<u8>().ok()) else {
                    continue;
                };

                colors.push(Color { r, g, b });
            }

            // number of colors should match number of tiles
            if colors.len() != total_colors {
                dropped_frames += 1;
                continue;
            }

            let mut crop = FrameCrop::init(
                tiles_x,
                resize_percentage,
                pos_x_percentage,
                pos_y_percentage);

            crop.colors = colors;

            let mut new_frame_added = false;

            let frame_data = frames
                .entry(frame_index)
                .or_insert_with(|| {
                    new_frame_added = true;
                    FrameData::new(frame_index)
                });

            if new_frame_added {
                total_frames_added += 1;
            }

            frame_data.crops.push(crop);

            if new_frame_added && total_frames_added % REPORT_PROGRESS_AFTER_FRAMES == 0 {
                tx.send(LoadDatabaseProgressReport {
                    video_file_name: video_file_name.clone(),
                    total_frames_processed: total_frames_added,
                    dropped_frames,
                    database: None,
                }).unwrap();
            }
        }

        let color_database = VideoColorIndexDatabase::new(
            tiles_x, tiles_y, frames.into_values().collect());

        tx.send(LoadDatabaseProgressReport {
            video_file_name: video_file_name,
            total_frames_processed: total_frames_added,
            dropped_frames,
            database: Some(color_database),
        }).unwrap();
    });

    rc
}


fn calculate_image_colors(app: &App) -> Receiver<Vec<ImageTile>> {
    let (tx, rc) = mpsc::channel::<Vec<ImageTile>>();

    let image = app.images.iter().find(|i| i.is_chosen)
        .expect("No image is chosen");
    let image_path = image.full_path.clone();

    let tiles_x = app.tiles_x;
    let tiles_y = app.tiles_y;

    let mosaic_tiles_x = app.mosaic_tiles_x;
    let mosaic_tiles_y = app.mosaic_tiles_y;

    thread::spawn(move || {
        let total_tiles = mosaic_tiles_x * mosaic_tiles_y;
        let mut image_tiles: Vec<ImageTile> = Vec::with_capacity(total_tiles as usize);

        let image_file = image::open(image_path).unwrap();
        let image_data = image_file.to_rgb8();

        let width: u32 = 7680;
        let height: u32 = 4320;

        // resize to 8K image to reduce chance of fractional units with large number of tiles/sub-titles
        let image_data = imageops::resize(
            &image_data, width, height, FilterType::Triangle);
    
        let tile_width = f64::round(width as f64 / mosaic_tiles_x as f64) as u32;
        let tile_height = f64::round(height as f64 / mosaic_tiles_y as f64) as u32;

        let sub_tile_width = f64::round(tile_width as f64 / tiles_x as f64) as u32;
        let sub_tile_height = f64::round(tile_height as f64 / tiles_y as f64) as u32;

        let total_sub_tile_pixels = sub_tile_width * sub_tile_height;

        for tile_y in 0..mosaic_tiles_y {
            for tile_x in 0..mosaic_tiles_x {
                let start_x = tile_x * tile_width;
                let start_y = tile_y * tile_height;

                let sub_image = image_data.view(start_x, start_y, tile_width, tile_height);

                let mut tile_data = ImageTile {
                    colors: vec![]
                };
                
                for sub_tile_y in 0..tiles_y {
                    for sub_tile_x in 0..tiles_x {
                        let start_x = sub_tile_x * sub_tile_width;
                        let end_x = cmp::min(start_x + sub_tile_width, tile_width);

                        let start_y = sub_tile_y * sub_tile_height;
                        let end_y = cmp::min(start_y + sub_tile_height, tile_height);

                        let mut total_red: u64 = 0;
                        let mut total_green: u64 = 0;
                        let mut total_blue: u64 = 0;

                        for pixel_y in start_y..end_y {
                            for pixel_x in start_x..end_x {
                                let pixel = sub_image.get_pixel(pixel_x, pixel_y);
                                let [red, green, blue] = pixel.0;

                                total_red += red as u64;
                                total_green += green as u64;
                                total_blue += blue as u64;
                            }
                        }

                        let average_red = f64::round(total_red as f64 / total_sub_tile_pixels as f64) as u64;
                        let average_green = f64::round(total_green as f64 / total_sub_tile_pixels as f64) as u64;
                        let average_blue = f64::round(total_blue as f64 / total_sub_tile_pixels as f64) as u64;

                        tile_data.colors.push(Color {
                            r: average_red as u8,
                            g: average_green as u8,
                            b: average_blue as u8,
                        });
                    }
                }

                image_tiles.push(tile_data);
            }
        }

        tx.send(image_tiles).unwrap();
    });

    rc
}

fn find_matches(app: &mut App) -> Receiver<FrameMatch> {
    let chosen_image = app.images.iter_mut()
        .find(|i| i.is_chosen && i.image_tiles.is_some());

    if let Some(chosen_image) = chosen_image {
        let mut matcher = ColorMatcher::new(app.mosaic_tiles_x, app.mosaic_tiles_y);

        matcher.set_thread_count(app.system_info.max_allowed_cores());

        let videos = app.videos.iter()
            .filter(|v| v.is_chosen && v.database.is_some());

        for video in videos {
            if let Some(database) = &video.database {
                matcher.add_database(&video.metadata.file_name, database);
            }
        }

        return matcher.match_tiles(chosen_image).unwrap();
    }

    panic!("No image");
}

struct MosaicGenerationReport {
    tile_index: u32,
    row: u32,
    col: u32,
}

fn generate_mosaic(app: &App) -> Result<Receiver<MosaicGenerationReport>> {
    let (progress_sender, progress_receiver) = mpsc::channel::<MosaicGenerationReport>();

    let image_result = app.images.iter()
        .find(|i| i.is_chosen);

    let Some(chosen_image) = image_result else {
        return Err(anyhow::format_err!("No chosen image found"));
    };

    let Some(frame_matches) = &chosen_image.matched_tiles else {
        return Err(anyhow::format_err!("Image has no matched tiles"));
    };

    // copy what we need
    let image_filename = chosen_image.file_name.clone();

    let mosaic_tiles_x = app.mosaic_tiles_x;
    let mosaic_tiles_y = app.mosaic_tiles_y;

    let frame_matches = frame_matches.clone();
    let mut video_filenames = frame_matches.iter()
        .map(|f| f.video_filename.clone())
        .collect::<Vec<_>>();

    video_filenames.dedup();

    let videos = app.videos.iter()
        .filter(|v| video_filenames.contains(&v.metadata.file_name))
        .map(|v| v.metadata.clone())
        .collect::<Vec<_>>();

    let working_dir = app.working_dir.clone();
    let database_dir = app.database_dir.clone();

    thread::spawn(move || {
        // create the database folder
        let database_dir_exists = fs::exists(&database_dir).unwrap_or(false);
        if !database_dir_exists {
            fs::create_dir(&database_dir).unwrap();
        }

        // create the temporary mosaic directory
        let temp_mosaic_dir_name = format!("{image_filename}_temp");
        let temp_mosaic_dir = database_dir.join(&temp_mosaic_dir_name);

        let temp_mosaic_exists = fs::exists(&temp_mosaic_dir).unwrap_or(false);
        if !temp_mosaic_exists {
            fs::create_dir(&temp_mosaic_dir).unwrap();
        }

        let (tx, rc) = mpsc::channel::<ImageTileData>();

        let mut video_frame_matches: HashMap<&str, Vec<VideoFrameMatch>> = HashMap::new();

        // group into videos
        for frame_match in &frame_matches {
            let entry = video_frame_matches
                .entry(frame_match.video_filename.as_str())
                .or_insert_with(|| Vec::new());

            entry.push(VideoFrameMatch {
                tile_index: frame_match.tile_index,
                frame_index: frame_match.frame_index,
                crop_resize: frame_match.crop_resize,
                crop_pos_x: frame_match.crop_pos_x,
                crop_pos_y: frame_match.crop_pos_y,
                is_flipped: frame_match.is_flipped,
            });
        }

        let mut workers: Vec<JoinHandle<()>> = vec![];

        for (video_filname, video_frame_matches) in video_frame_matches {
            if let Some(video) = videos.iter().find(|v| v.file_name == video_filname) {
                let video_metadata = video.clone();
                let tx = tx.clone();

                workers.push(thread::spawn(move || {
                    let mut frame_extractor = FrameExtractor::new(0, video_metadata);
                    frame_extractor.set_max_threads(18);
                    frame_extractor.run(&video_frame_matches, tx).unwrap();
                }));
            }
        }

        drop(tx);

        for tile in rc {
            let row = tile.tile_index / mosaic_tiles_x;
            let col = tile.tile_index % mosaic_tiles_x;

            let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
            tile.data.save(tile_path).unwrap();

            progress_sender.send(MosaicGenerationReport {
                tile_index: tile.tile_index,
                row,
                col,
            }).unwrap();
        }

        for worker in workers {
            worker.join().unwrap();
        }

        // join images
        let image_width: u32 = 7680;
        let image_height: u32 = 4320;

        let ratio = image_width as f64 / image_height as f64;

        const LARGEST_DIMENSION: u32 = 10000;
        let smallest_dimension = (LARGEST_DIMENSION as f64 / ratio).round() as u32;

        let is_landscape = image_width > image_height;

        let target_width = if is_landscape { LARGEST_DIMENSION } else { smallest_dimension };
        //let target_height = if is_landscape { smallest_dimension } else { LARGEST_DIMENSION };

        let tile_width = (target_width as f64 / mosaic_tiles_x as f64).round() as u32;
        let tile_height = (tile_width as f64 / ratio as f64).round() as u32;

        let final_width = tile_width * mosaic_tiles_x;
        let final_height = tile_height * mosaic_tiles_y;
        
        let mut canvas: ImageBuffer<Rgb<u8>, Vec<u8>> = RgbImage::new(final_width, final_height);

        for frame_match in frame_matches {
            let row = frame_match.tile_index / mosaic_tiles_x;
            let col = frame_match.tile_index % mosaic_tiles_x;

            let tile_path = temp_mosaic_dir.join(format!("{row}x{col}.png"));
            let tile_image = image::open(tile_path).unwrap();
            let image = tile_image.as_rgb8().unwrap();
            let resized = imageops::resize(
                image, tile_width, tile_height, FilterType::Triangle);

            canvas.copy_from(&resized, col * tile_width, row * tile_height).unwrap();
        }

        let mosaic_image_name = format!("{image_filename}_mosaic_{mosaic_tiles_x}x{mosaic_tiles_y}.png");
        let image_path = working_dir.join(&mosaic_image_name);
        canvas.save(image_path).unwrap();
    });

    Ok(progress_receiver)
}