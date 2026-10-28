use crate::app::frame_data::FrameData;

pub struct VideoColorIndexDatabase {
    pub color_tiles_x: u32,
    pub color_tiles_y: u32,

    frames: Vec<FrameData>,
}

impl VideoColorIndexDatabase {
    pub fn new(color_tiles_x: u32, color_tiles_y: u32, frames: Vec<FrameData>) -> VideoColorIndexDatabase {
        VideoColorIndexDatabase {
            color_tiles_x,
            color_tiles_y,
            frames,
        }
    }

    pub fn frames(&self) -> impl Iterator<Item = &FrameData> + '_ { // avoiding memory allocation for large videos
        self.frames.iter()
    }

    pub fn total_frames(&self) -> u64 {
        self.frames.len() as u64
    }
}