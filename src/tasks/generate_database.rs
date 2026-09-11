use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::fs::{self, File};

use crate::app::{
    App,
    VideoIndexCore,
    VideoIndexStatus,
    VideoIndexingReport
};

use crate::ffmpeg::{
    VideoMetadata, 
    color_extractor::{
        ColorExtractionProgress,
        ColorExtractor
    }
};

pub fn run(video: &VideoMetadata, app: &App) -> Receiver<VideoIndexingReport> {
    let (video_progress_sender, video_progress_receiver) =
        mpsc::channel::<VideoIndexingReport>();

    let database_dir = app.database_dir.clone();

    let max_allowed_cores = app.system_info.max_allowed_cores();

    let color_tiles_x = app.color_tiles_x;
    let color_tiles_y = app.color_tiles_y;

    let video = video.clone();
    let color_extracion_algorithm = app.color_extraction_algorithm.clone();

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

            let color_extracion_algorithm = color_extracion_algorithm.clone();
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
                    color_tiles_x,
                    color_tiles_y,
                    temp_file_path.as_path()).unwrap();

                extractor.set_algorithm(color_extracion_algorithm);
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

                let is_core_finished = core.total_frames == core.frames_processed;
                core.status = if is_core_finished { VideoIndexStatus::Finished } else { VideoIndexStatus::Running };
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
        // wk_dir/pmg_data/my_holiday.mp4-5x5.pmgd
        let data_file_name = format!("{}-{}x{}.pmgd", video.file_name, color_tiles_x, color_tiles_y);
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