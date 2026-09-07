use std::{collections::HashMap, path::{Path, PathBuf}, process::{Command, Stdio}, str::FromStr, time::Duration};
use anyhow::{Context, Result};

use crate::ffmpeg::{AspectRatio, VideoCodec};

#[derive(Clone, Debug)]
pub struct VideoMetadata {
    pub file_name: String,
    pub full_path: PathBuf,

    pub width: u32,
    pub height: u32,
    pub aspect_ratio: AspectRatio,
    pub pixels_per_frame: u32,

    pub frame_rate: f64,
    pub is_variable_frame_rate: bool,
    pub total_frames: u64,
    pub duration: Duration,

    pub total_bytes: u64,
    pub bit_rate: u64,

    pub codec: VideoCodec,
}

impl VideoMetadata {
    pub fn extract_from(video_path: &Path) -> Result<Self> {
        let file_exists = std::fs::exists(video_path)?;
        if !file_exists {
            return Err(anyhow::format_err!("`{}` does not exist", video_path.display()));
        }

        let meta_ouput = Command::new("ffprobe")
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height,r_frame_rate,avg_frame_rate,nb_frames,bit_rate,codec_name:format=size,duration",
                "-of", "default=noprint_wrappers=1",
            ])
            .arg(video_path)
            .stdout(Stdio::piped())
            .output()?;

        let output = String::from_utf8(meta_ouput.stdout)?;
        let video_metadata = Self::from_stdout(video_path, &output)?;

        Ok(video_metadata)
    }

    fn from_stdout(video_path: &Path, stdout: &str) -> Result<Self> {
        let file_name = video_path.file_name()
            .with_context(|| format!("can't extract a file name from `{}`", video_path.display()))?
            .display()
            .to_string();

        let mut meta_items: HashMap<String, String> = HashMap::new();
        
        let items: Vec<&str> = stdout.split("\n").collect();
        for item in items {
            if let Some((key, value)) = item.split_once('=') {
                meta_items.insert(key.to_string(), value.to_string());
            }
        }

        let width: u32 = Self::get_property("width", &meta_items)?;
        let height: u32 = Self::get_property("height", &meta_items)?;

        let frame_rate_raw = meta_items.get("r_frame_rate")
            .with_context(|| format!("video meta-data is missing `r_frame_rate"))?;
        let average_frame_rate_raw = meta_items.get("avg_frame_rate")
            .with_context(|| format!("video meta-data is missing `avg_frame_rate"))?;

        let is_variable_frame_rate = frame_rate_raw != average_frame_rate_raw;

        let frame_rate = Self::get_frame_rate(frame_rate_raw)?;
        let duration_secs: f64 = Self::get_property("duration", &meta_items)?;
        
        let total_frames = match Self::get_property::<u64>("nb_frames", &meta_items).ok() {
            Some(total_frames) => total_frames,
            None =>  (duration_secs * frame_rate).round() as u64,
        };

        let total_bytes: u64 = Self::get_property("size", &meta_items)?;

        let bit_rate_result = Self::get_property("bit_rate", &meta_items);
        let bit_rate = match bit_rate_result {
            Ok(bit_rate) => bit_rate,
            Err(_) => {
                // can sometimes be N/A, so estimate based on filesize
                // subtract audio data, only needs to be approx
                let average_audio_bit_rate: f64 = 256000.0; // 256 kbps
                let audio_bytes = (average_audio_bit_rate * duration_secs).round() as u64;
                let video_bytes = total_bytes - audio_bytes;

                let bit_rate = f64::round((video_bytes as f64 * 8.0) / duration_secs as f64) as u64;
                bit_rate
            }
        };

        let codec_raw: String = Self::get_property("codec_name", &meta_items)?;
        let codec = VideoCodec::from_ffmpeg_meta_data(&codec_raw);

        let aspect_ratio = AspectRatio::new(width, height);
        let pixels_per_frame = width * height;
        let duration = Duration::from_secs_f64(duration_secs);
        
        Ok(Self {
            file_name: String::from(file_name),
            full_path: PathBuf::from(video_path),
            width,
            height,
            aspect_ratio,
            pixels_per_frame,
            frame_rate,
            is_variable_frame_rate,
            total_frames,
            duration,
            total_bytes,
            bit_rate,
            codec
        })
    }

    fn get_property<T>(prop: &str, meta_items: &HashMap<String, String>) -> Result<T>
    where 
        T: FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let value: T = meta_items.get(prop)
            .with_context(|| format!("video meta-data is missing `{prop}`"))?
            .parse::<T>()
            .with_context(|| format!("video meta_data `{prop}` is not a valid type"))?;

        Ok(value)
    }

    fn get_frame_rate(input: &str) -> Result<f64> {
        let fps_parts: Vec<&str> = input.split("/").collect();
        let fps_first: u32 = fps_parts[0].parse()?;
        let fps_last: u32 = fps_parts[1].parse()?;

        let frame_rate: f64 = fps_first as f64 / fps_last as f64;
        // note don't round, e.g 29.97002997002997 > 29.97 as the extra
        // precision is needed for accurate individual frame selection

        Ok(frame_rate)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    const DIR: &str = "my_dir/sub_dir/";
    const FILE_NAME: &str = "my_video.mp4";

    #[test]
    fn cfr_h264() {
        let ffprobe_output = indoc! {"
            codec_name=h264
            width=1920
            height=1080
            r_frame_rate=30/1
            avg_frame_rate=30/1
            bit_rate=62043742
            nb_frames=55764
            duration=1860.658800
            size=14498523902
        "};

        let path = PathBuf::from(DIR).join(FILE_NAME);

        let metadata = VideoMetadata::from_stdout(&path, ffprobe_output)
            .expect("should not throw error");

        assert_eq!(metadata.file_name, FILE_NAME);
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert_eq!(metadata.frame_rate, 30.0);
        assert_eq!(metadata.is_variable_frame_rate, false);
        assert_eq!(metadata.bit_rate, 62043742);
        assert_eq!(metadata.total_frames, 55764);
        assert_eq!(metadata.total_bytes, 14498523902);
        assert_eq!(metadata.aspect_ratio, AspectRatio::Landscape16x9);
        assert_eq!(metadata.codec, VideoCodec::H264);

        let duration = Duration::from_secs_f64(1860.658800);
        assert_eq!(metadata.duration, duration);
    }

    #[test]
    fn vrf_hevc() {
        let ffprobe_output = indoc! {"
            codec_name=hevc
            width=3840
            height=2880
            r_frame_rate=30000/1001
            avg_frame_rate=54675/1823
            bit_rate=23101830
            nb_frames=7290
            duration=243.066667
            size=709694454
        "};

        let path = PathBuf::from(DIR).join(FILE_NAME);

        let metadata = VideoMetadata::from_stdout(&path, ffprobe_output)
            .expect("should not throw error");

        assert_eq!(metadata.file_name, FILE_NAME);
        assert_eq!(metadata.width, 3840);
        assert_eq!(metadata.height, 2880);
        assert_eq!(metadata.frame_rate, 29.97002997002997);
        assert_eq!(metadata.is_variable_frame_rate, true);
        assert_eq!(metadata.bit_rate, 23101830);
        assert_eq!(metadata.total_frames, 7290);
        assert_eq!(metadata.total_bytes, 709694454);
        assert_eq!(metadata.aspect_ratio, AspectRatio::Landscape4x3);
        assert_eq!(metadata.codec, VideoCodec::HEVC);

        let duration = Duration::from_secs_f64(243.066667);
        assert_eq!(metadata.duration, duration);
    }

    #[test]
    fn frame_count_missing_av1() {
        let ffprobe_output = indoc! {"
            codec_name=av1
            width=3840
            height=2160
            r_frame_rate=60/1
            avg_frame_rate=60/1
            bit_rate=N/A
            nb_frames=N/A
            duration=931.268000
            size=3064885615
        "};

        let path = PathBuf::from(DIR).join(FILE_NAME);

        let metadata = VideoMetadata::from_stdout(&path, ffprobe_output)
            .expect("should not throw error");

        assert_eq!(metadata.file_name, FILE_NAME);
        assert_eq!(metadata.width, 3840);
        assert_eq!(metadata.height, 2160);
        assert_eq!(metadata.frame_rate, 60.0);
        assert_eq!(metadata.is_variable_frame_rate, false);
        assert_eq!(metadata.bit_rate, 24280710);
        assert_eq!(metadata.total_frames, 55876);
        assert_eq!(metadata.total_bytes, 3064885615);
        assert_eq!(metadata.aspect_ratio, AspectRatio::Landscape16x9);
        assert_eq!(metadata.codec, VideoCodec::AV1);
    }
}