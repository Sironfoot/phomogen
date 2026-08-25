pub mod summed_table;
pub use summed_table::SummedAreaTable;

pub mod pixel_array;
pub use pixel_array::PixelArray;

use std::{fs::File, io::{BufWriter, Read}, process::{Command, Stdio}, sync::mpsc, time::Instant};
use anyhow::Result;
use std::io::Write as IoWrite;

use crate::ffmpeg::VideoMetadata;
use crate::ffmpeg::crops::CropSetting;

const BYTES_PER_PIXEL: u32 = 3;

#[derive(PartialEq, Debug)]
pub enum ColorExtractionAlgorithm {
    PixelArrayTraversal,
    SummedAreaTable,
}

pub trait FrameColorExtractionAlgorithm {
    fn process_frame(&mut self, frame_number: u64, pixels: &[u8]) -> Result<String>;
}

fn compute_output_buffer_size(crops: &[CropSetting]) -> usize {
    let mut buffer_capacity: usize = 10;

    // database entries look like: [Frame Index] [Crop Size] [PosX] [PosY] [R,G,B] [R,G,B] [R,G,B] [R,G,B]...
    // e.g. 1123456 66.666 33.333 33.333 255,255,255 255,255,255 255,255,255 255,255,255....
    let prefix_length = "1123456 66.666 33.333 33.333 ".len();
    let rgb_length = "255,255,255 ".len();
    
    for crop in crops.iter() {
        buffer_capacity += prefix_length;

        let num_tiles = crop.tiles.len();
        buffer_capacity += num_tiles * rgb_length;
    }

    buffer_capacity
}

pub struct ColorExtractionProgress {
    pub core_id: u32,
    pub total_frames_processed: u64,
    pub average_fps: f64,
    pub percentage_complete: f64,
}

pub struct ColorExtractor {
    core_id: u32,
    data_file: BufWriter<File>,

    video_meta: VideoMetadata,
    video_path: String,
    max_ffmpeg_threads: u32,
    resize_width: u32,

    start_frame_index: u64,
    end_frame_index: u64,

    tiles_x: u32,
    tiles_y: u32,

    algorithm: ColorExtractionAlgorithm,
}

impl ColorExtractor {
    pub fn init(
        video_meta: VideoMetadata,
        video_path: String,
        core_id: u32,
        start_frame_index: u64,
        end_frame_index: u64,
        tiles_x: u32,
        tiles_y: u32,
        data_file_path: String) -> Result<ColorExtractor> {

        if video_meta.is_variable_frame_rate {
            return Err(anyhow::format_err!("variable frame rate videos not supported"));
        }

        let file = File::create(data_file_path)?;
        let data_file = BufWriter::new(file);

        Ok(ColorExtractor {
            core_id,
            data_file,
            video_meta: video_meta,
            video_path,
            max_ffmpeg_threads: 4,
            resize_width: 1920,
            start_frame_index,
            end_frame_index,
            tiles_x,
            tiles_y,
            algorithm: ColorExtractionAlgorithm::SummedAreaTable,
        })
    }

    pub fn set_max_threads(&mut self, num: u32) -> &mut Self {
        let num = if num == 0 { 1 } else { num };

        self.max_ffmpeg_threads = num;
        return self;
    }

    pub fn set_resize_width(&mut self, width: u32) -> &mut Self {
        let width = if width < 640 { 640 } else { width };

        self.resize_width = width;
        return self;
    }

    pub fn set_algorithm(&mut self, algorithm: ColorExtractionAlgorithm) -> &mut Self {
        self.algorithm = algorithm;
        return self;
    }

    pub fn run(&mut self, tx: mpsc::Sender<ColorExtractionProgress>) -> Result<()> {
        let frame_width = self.resize_width;
        let frame_height = (frame_width as f64 / self.video_meta.aspect_ratio.ratio()).round() as u32;
        
        let frame_crops = CropSetting::all_crops(frame_width, frame_height, self.tiles_x, self.tiles_y)?;
        
        let seconds_to_target_frame = self.start_frame_index as f64 / self.video_meta.frame_rate;

        let mut child = Command::new("ffmpeg")
            .args([
                "-threads", &format!("{}", self.max_ffmpeg_threads),
                "-ss", &format!("{seconds_to_target_frame}"),
                "-i", &self.video_path,

                "-vf", &format!("scale={}:-2:flags=area", self.resize_width),

                // No audio/subtitles/data output
                "-an",
                "-sn",
                "-dn",

                // Raw RGB pixels
                "-f", "rawvideo",
                "-pix_fmt", "rgb24",

                // Output to stdout
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        
        let frame_size = frame_width * frame_height * BYTES_PER_PIXEL;

        let mut stdout = child.stdout.take().unwrap();
        let mut buffer = vec![0u8; frame_size as usize];

        let mut current_frame_index = self.start_frame_index;
        let total_frames = self.end_frame_index - self.start_frame_index;

        let average_timer = Instant::now();
        let output_frame_interval = 123;

        let mut color_extraction_algorithm: Box<dyn FrameColorExtractionAlgorithm> =
            match self.algorithm {
                ColorExtractionAlgorithm::PixelArrayTraversal => {
                    Box::new(PixelArray::new(frame_width, frame_crops))
                },
                ColorExtractionAlgorithm::SummedAreaTable => {
                    Box::new(SummedAreaTable::new(frame_width, frame_height, frame_crops))
                }
            };

        loop {
            match stdout.read_exact(&mut buffer) {
                Ok(()) => {
                    let output = color_extraction_algorithm
                        .process_frame(current_frame_index, &buffer)?;

                    self.data_file.write_all(output.as_bytes())?;

                    current_frame_index += 1;

                    if current_frame_index == self.end_frame_index {
                        tx.send(ColorExtractionProgress {
                            core_id: self.core_id,
                            total_frames_processed: total_frames,
                            average_fps: 0.0,
                            percentage_complete: 100.0,
                        }).unwrap();

                        child.kill().unwrap();
                        break;
                    }

                    if current_frame_index % output_frame_interval == 0 {
                        let total_frames_processed = current_frame_index - self.start_frame_index;
                        let percentage_complete = (total_frames_processed as f64 / self.video_meta.frame_rate as f64) * 100.0;

                        let average_elapsed = average_timer.elapsed().as_secs_f64();
                        let average_fps = total_frames_processed as f64 / average_elapsed;

                        tx.send(ColorExtractionProgress {
                            core_id: self.core_id,
                            total_frames_processed: total_frames_processed,
                            average_fps: average_fps,
                            percentage_complete: percentage_complete,
                        }).unwrap();
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    tx.send(ColorExtractionProgress {
                        core_id: self.core_id,
                        total_frames_processed: total_frames,
                        average_fps: 0.0,
                        percentage_complete: 100.0,
                    }).unwrap();
                    
                    break;
                },
                Err(error) => {
                    return Err(anyhow::format_err!("Failed reading ffmpeg output: {error}"));
                }
            }
        }

        child.wait()?;

        Ok(())
    }
}