pub struct VideoColorIndexDatabase {
    pub tiles_x: u32,
    pub tiles_y: u32,

    frames: Vec<FrameData>,
}

impl VideoColorIndexDatabase {
    pub fn new(tiles_x: u32, tiles_y: u32, frames: Vec<FrameData>) -> VideoColorIndexDatabase {
        VideoColorIndexDatabase {
            tiles_x,
            tiles_y,
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

#[derive(Debug)]
pub struct FrameData {
    pub frame_index: u32,
    pub crops: Vec<FrameCrop>,
}

impl FrameData {
    pub fn new(frame_index: u32) -> FrameData {
        FrameData {
            frame_index,
            crops: vec![],
        }
    }
}

#[derive(Debug)]
pub struct FrameCrop {
    pub resize_percentage: f64,
    pub pos_x_percentage: f64,
    pub pos_y_percentage: f64,

    tiles_x: usize,
    pub colors: Vec<Color>,
}

impl FrameCrop {
    pub fn init(tiles_x: u32, resize_percentage: f64, pos_x_percentage: f64, pos_y_percentage: f64) -> FrameCrop {
        FrameCrop {
            resize_percentage,
            pos_x_percentage,
            pos_y_percentage,
            tiles_x: tiles_x as usize,
            colors: vec![]
        }
    }

    pub fn colors_flipped(&self) -> impl Iterator<Item = &Color> + '_ {
        self.colors
            .chunks_exact(self.tiles_x)
            .flat_map(|row| row.iter().rev())
    }
}

#[derive(Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}