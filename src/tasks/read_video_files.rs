use std::{fs, sync::mpsc::{self, Receiver}, thread};

use crate::{app::{App, VideoFile}, ffmpeg::VideoMetadata};

pub fn run(app: &App) -> Receiver<Vec<VideoFile>> {
    let (tx, rc) = mpsc::channel::<Vec<VideoFile>>();
    let working_dir = app.working_dir.clone();
    let database_dir = app.database_dir.clone();

    let color_tiles_x = app.color_tiles_x;
    let color_tiles_y = app.color_tiles_y;

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

            let data_file = format!("{video_file}-{}x{}.pmgd", color_tiles_x, color_tiles_y);
            let full_data_path = database_dir.join(&data_file);

            let data_exists = fs::exists(&full_data_path).unwrap_or(false);

            if let Ok(meta_data) = meta_data {
                let mut video = VideoFile::new(meta_data);
                video.database_path = if data_exists { Some(full_data_path) } else { None };

                videos.push(video);
            }
        }

        videos.sort_by_cached_key(|i| i.metadata.file_name.to_lowercase());

        tx.send(videos).unwrap();
    });

    rc
}