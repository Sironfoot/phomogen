#[derive(PartialEq, Clone, Debug)]
pub enum VideoCodec {
    H264,
    HEVC,
    AV1,
    VP9,
    ProRes,
    MPEG2,
    Other(String),
}

impl VideoCodec {
    pub fn from_ffmpeg_meta_data(codec: &str) -> Self {
        match codec.to_lowercase().as_str() {
            "h264" => Self::H264,
            "hevc" => Self::HEVC,
            "av1" => Self::AV1,
            "vp9" => Self::VP9,
            "prores" => Self::ProRes,
            "mpeg2video" => Self::MPEG2,
            codec => Self::Other(codec.to_string()),
        }
    }
}