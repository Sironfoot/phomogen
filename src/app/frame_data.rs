use std::collections::HashMap;

pub struct VideoColorIndexDatabase {
    pub tiles_x: u32,
    pub tiles_y: u32,
    num_tiles: u32,

    frames: HashMap<u64, FrameData>,
}

impl VideoColorIndexDatabase {
    pub fn new(tiles_x: u32, tiles_y: u32, total_frames: u64) -> VideoColorIndexDatabase {
        let num_tiles = tiles_x * tiles_y;

        VideoColorIndexDatabase {
            tiles_x,
            tiles_y,
            num_tiles,
            frames: HashMap::with_capacity(total_frames as usize),
        }
    }

    pub fn add_frame_crop(&mut self, frame_index: u64, frame_crop: FrameCrop) -> Result<bool, AddFrameCropError> {
        let frame_crops_tiles = frame_crop.colors.len() as u32;

        // since the database is a basic text file, we needto deal with the potential
        // that it's been opened and tampered with
        if self.num_tiles != frame_crops_tiles {
            return Err(AddFrameCropError::WrongTileCount);
        }

        if frame_crop.resize_percentage < 1.0 || frame_crop.resize_percentage > 100.0 {
            return Err(AddFrameCropError::InvalidResizePercentage);
        }

        if frame_crop.pos_x_percentage > 100.0 {
            return Err(AddFrameCropError::InvalidPosX);
        }

        if frame_crop.pos_y_percentage > 100.0 {
            return Err(AddFrameCropError::InvalidPosY);
        }

        let mut new_frame_inserted = false;

        let frame_data = self.frames
            .entry(frame_index)
            .or_insert_with(|| {
                new_frame_inserted = true;
                FrameData::new(frame_index)
            });

        frame_data.crops.push(frame_crop);

        Ok(new_frame_inserted)
    }

    pub fn frames(&self) -> impl Iterator<Item = &FrameData> + '_ { // avoiding memory allocation for large videos
        self.frames.values()
    }

    pub fn total_frames(&self) -> u64 {
        self.frames.len() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddFrameCropError {
    WrongTileCount,
    InvalidResizePercentage,
    InvalidPosX,
    InvalidPosY,
}

#[derive(Debug)]
pub struct FrameData {
    pub frame_index: u64,
    pub crops: Vec<FrameCrop>,
}

impl FrameData {
    fn new(frame_index: u64) -> FrameData {
        FrameData {
            frame_index,
            crops: vec![],
        }
    }

    // pub fn flip(&self) -> FrameData {
    //     let mut flipped_frame = FrameData {
    //         tiles_x: self.tiles_x,
    //         tiles_y: self.tiles_y,

    //         frame_index: self.frame_index,
    //         crops: Vec::with_capacity(self.crops.len()),
    //     };
        
    //     for crop in &self.crops {
    //         let mut flipped_crop = FrameCrop {
    //             resize_percentage: crop.resize_percentage,
    //             pos_x_percentage: crop.pos_x_percentage,
    //             pos_y_percentage: crop.pos_y_percentage,
    //             colors: Vec::with_capacity(crop.colors.len()),
    //         };

    //         for tile_y in 0..self.tiles_y {
    //             for tile_x in (0..self.tiles_x).rev() {
    //                 let offset = (tile_y * self.tiles_x + tile_x) as usize;
    //                 let color = &crop.colors[offset];

    //                 flipped_crop.colors.push(Color { r: color.r, g: color.g, b: color.b });
    //             }
    //         }

    //         flipped_frame.crops.push(flipped_crop);
    //     }

    //     flipped_frame
    // }
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