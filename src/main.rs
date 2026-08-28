pub mod app;
pub mod ui;
pub mod ffmpeg;

use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{io, thread};
use std::fs::{self, File};

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

use crate::app::{App, AppStage, ImageFile, ImageType, SystemInfo, VideoFile, VideoIndexCore, VideoIndexStatus, VideoIndexingReport};
use crate::ffmpeg::color_extractor::{ColorExtractionAlgorithm, ColorExtractionProgress, ColorExtractor};
use crate::ui::render_ui;
use crate::ffmpeg::VideoMetadata;

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
                // TODO: hot reloadding of video list
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
                // TODO: implement
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
                    is_chosen: false,
                    database_path: None,
                    indexing_report: None,
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
                is_chosen: false,
            });
            
        }

        tx.send(images).unwrap();
    });

    rc
}