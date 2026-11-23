use anyhow::Result;
use crate::ffmpeg::AspectRatio;

/// Represents how a frame from a video should be divided up into a tile grid for calculating color averages
/// (for later calculating the Euclidean distance for matching), while also supporting cropping on the frame,
/// given a specific video resolution and crop amount. 
/// 
/// For instance, if we want to divide a 16x9 frame into 8x8 segments, but our cropping level is 50% of the frame, we would divide
/// the full frame into 16x16 segments. This 16x16 grid is what gets stored in the database. Later we need the top left corner we return
/// the x 0-8 and y 0-8 tiles, then keep shifting the crop along the 16x16 grid which would yield 81 crops per frame.
/// 
/// Ensures all tile x,y coords perfectly sit on pixels boundaries with no rounding for accurate color averaging,
/// even if the top/bottom or left/right of the get cropped very slightly. For example: 1920x1080 frame with 16x16
/// tiles the height per tile would 67.5px, we always round down to 67px but then the total height would be 1072px,
/// so 4px from the top and 4px from the bottom are cropped and are never used in calculations, and also not used
/// when the frame is eventually pulled from the video to use in the Mosaic
#[derive(Debug, Clone)]
pub struct CropSetting {
    resize_percentage: u8,
    pub crop_level: CropLevel,

    color_tiles_x: u8,
    color_tiles_y: u8,

    full_color_tiles_x: u32,
    full_color_tiles_y: u32,

    tile_right_shift: u8,
    tile_down_shift: u8,

    tile_width: u32,
    tile_height: u32,
    tile_pixels: u32,

    tiles: Vec<CropTile>,
}

impl CropSetting {
    pub fn resize_percentage(&self) -> u8 {
        self.resize_percentage
    }

    pub fn color_tiles_x(&self) -> u8 {
        self.color_tiles_x
    }

    pub fn color_tiles_y(&self) -> u8 {
        self.color_tiles_y
    }

    pub fn full_color_tiles_x(&self) -> u32 {
        self.full_color_tiles_x
    }

    pub fn full_color_tiles_y(&self) -> u32 {
        self.full_color_tiles_y
    }

    pub fn tile_dimensions(&self) -> (u32, u32) {
        (self.tile_width, self.tile_height)
    }

    pub fn total_tile_pixels(&self) -> u32 {
        self.tile_pixels
    }

    pub fn tile_right_shift(&self) -> u8 {
        self.tile_right_shift
    }

    pub fn tile_down_shift(&self) -> u8 {
        self.tile_down_shift
    }

    pub fn total_crops(&self) -> u32 {
        self.tile_right_shift as u32 * self.tile_down_shift as u32
    }

    pub fn tiles(&self) -> &[CropTile] {
        &self.tiles
    }

    pub fn extract_crop_tile(&self, start_x: u8, start_y: u8) -> CropTile {
        let top_left_index = ((start_y as u32 * self.full_color_tiles_x) + start_x as u32) as usize;
        let top_left_tile = &self.tiles[top_left_index];

        let width = self.color_tiles_x as u32 * self.tile_width;
        let height = self.color_tiles_y as u32 * self.tile_height;

        CropTile {
            start_x: top_left_tile.start_x,
            start_y: top_left_tile.start_y,
            end_x: top_left_tile.start_x + width,
            end_y: top_left_tile.start_y + height,
        }
    }

    fn new16x9(resize: u8, crop_level: CropLevel, frame_width: u32, frame_height: u32, color_tiles_x: u8, color_tiles_y: u8) -> Self {
        // example: we're using 9x9 color tiles to extract RGB averages from, and the cropped resize we're using is 50%
        // we therefore devide the entire video frame into 18x18 grid. When it comes time to perform nearest matches for the Mosaic
        // and we need the top-left crop of the frame we grab the top-left 9x9 tiles from that 18x18 grid.
        let full_color_tiles_x = ((100.0 / resize as f64) * color_tiles_x as f64).round() as u32;
        let full_color_tiles_y = ((100.0 / resize as f64) * color_tiles_y as f64).round() as u32;

        let tile_width = (frame_width as f64 / full_color_tiles_x as f64).floor() as u32;
        let tile_height = (frame_height as f64 / full_color_tiles_y as f64).floor() as u32;
        let tile_pixels = tile_width * tile_height;

        // we want pixel perfect tiles that fit perfectly on pixel boundaries to accurately extract color data from
        // 1920px width / 18 color tiles = 106.66666667, so we make color tile width 106px (always round down)
        // but 106 * 18 = 1908, so there are 12 pixels being ignored so we ignore 6 pixels off left and right edges
        // |--6px--|----106px----|----106px----|----106px----|....snip....|----106px----|--6px--|
        let cropped_width = tile_width * full_color_tiles_x as u32;
        let cropped_start_x: u32 = if cropped_width < frame_width {
            ((frame_width - cropped_width) as f64 / 2.0).floor() as u32
        } else { 0 };

        let cropped_height = tile_height * full_color_tiles_y as u32;
        let cropped_start_y: u32 = if cropped_height < frame_height {
            ((frame_height - cropped_height) as f64 / 2.0).floor() as u32
        } else { 0 };

        let mut tiles: Vec<CropTile> = Vec::with_capacity((color_tiles_x * color_tiles_y) as usize);

        for grid_y in 0..full_color_tiles_y {
            for grid_x in 0..full_color_tiles_x {
                let start_x = (grid_x * tile_width) + cropped_start_x;
                let end_x = start_x + tile_width;

                let start_y = (grid_y * tile_height) + cropped_start_y;
                let end_y = start_y + tile_height;

                tiles.push(CropTile { start_x, end_x, start_y, end_y });
            }
        }

        let tile_right_shift = (full_color_tiles_x as u8 - color_tiles_x) + 1;
        let tile_down_shift = (full_color_tiles_y as u8 - color_tiles_y) + 1;
        
        Self {
            resize_percentage: resize,
            crop_level,
            color_tiles_x,
            color_tiles_y,
            full_color_tiles_x,
            full_color_tiles_y,
            tile_right_shift,
            tile_down_shift,
            tile_width,
            tile_height,
            tile_pixels,
            tiles,
        }
    }

    pub fn all_crops(frame_width: u32, frame_height: u32, color_tiles_x: u8, color_tiles_y: u8) -> Result<Vec<Self>> {
        let aspect_ratio = AspectRatio::new(frame_width, frame_height);

        match aspect_ratio {
            AspectRatio::Landscape16x9 => {
                Ok(Self::get_all_16x9(frame_width, frame_height, color_tiles_x, color_tiles_y))
            },
            _ => {
                Err(anyhow::format_err!("`{aspect_ratio}` aspect ratio videos aren't supported"))
            }
        }
    }

    fn get_all_16x9(frame_width: u32, frame_height: u32, color_tiles_x: u8, color_tiles_y: u8) -> Vec<Self> {
        vec![
            Self::new16x9(50, CropLevel::Aggressive, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new16x9(60, CropLevel::Aggressive, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new16x9(70, CropLevel::Moderate, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new16x9(80, CropLevel::Moderate, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new16x9(90, CropLevel::Moderate, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new16x9(100, CropLevel::Essential, frame_width, frame_height, color_tiles_x, color_tiles_y),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct CropTile {
    pub start_x: u32,
    pub end_x: u32,
    pub start_y: u32,
    pub end_y: u32,
}

impl CropTile {
    pub fn dimensions(&self) -> (u32, u32) {
        let width = self.end_x - self.start_x;
        let height = self.end_y - self.start_y;

        (width, height)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropLevel {
    Essential = 1,
    Moderate = 2,
    Aggressive = 3,
}

impl TryFrom<u8> for CropLevel {
    type Error = u8;

    fn try_from(value: u8) -> std::prelude::v1::Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Essential),
            2 => Ok(Self::Moderate),
            3 => Ok(Self::Aggressive),
            invalid => Err(invalid),
        }
    }
}

// 104 vs 232

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1920_1080_with_8x8() {
        let frame_width: u32 = 1920;
        let frame_height: u32 = 1080;

        let color_tiles_x: u8 = 8;
        let color_tiles_y: u8 = 8;

        // full frame
        let full_frame = CropSetting::new16x9(
            100,
            CropLevel::Essential,
            frame_width,
            frame_height,
            color_tiles_x,
            color_tiles_y);

        assert_eq!(full_frame.full_color_tiles_x(), 8, "full_frame: full_color_tiles_x should be 8");
        assert_eq!(full_frame.full_color_tiles_y(), 8, "full_frame: full_color_tiles_y should be 8");

        let expected_tile_dimensions: (u32, u32) = (240, 135);
        let (tile_width, tile_height) = expected_tile_dimensions;
        assert_eq!(full_frame.tile_dimensions(), expected_tile_dimensions, "full_frame: expected tile dimensions should be {expected_tile_dimensions:?}");

        let expected_tile_pixels = (tile_width * tile_height) as u32;
        assert_eq!(full_frame.total_tile_pixels(), expected_tile_pixels, "full_frame: expected tile pixels should be {expected_tile_pixels}");

        let expected_tiles = (full_frame.full_color_tiles_x() * full_frame.full_color_tiles_y()) as usize;
        assert_eq!(full_frame.tiles.len(), expected_tiles, "full_frame: should be {expected_tiles} tile entries");

        let first_tile = full_frame.tiles.first().unwrap();
        assert_eq!(first_tile.start_x, 0, "full_frame: first tile start_x should be 0");
        assert_eq!(first_tile.end_x, tile_width, "full_frame: first tile end_x should be {}", tile_width);
        assert_eq!(first_tile.start_y, 0, "full_frame: first tile start_y should be 0");
        assert_eq!(first_tile.end_y, tile_height, "full_frame: first tile end_xyshould be {}", tile_height);

        let last_tile = full_frame.tiles.last().unwrap();
        assert_eq!(last_tile.start_x, frame_width - tile_width, "full_frame: last tile start_x should be {}", frame_width - tile_width);
        assert_eq!(last_tile.end_x, frame_width, "full_frame: last tile end_x should be {frame_width}");
        assert_eq!(last_tile.start_y, frame_height - tile_height, "full_frame: last tile start_y should be {}", frame_height - tile_height);
        assert_eq!(last_tile.end_y, frame_height, "full_frame: last tile end_y should be {frame_height}");

        // 50% crop
        let fifty_crop = CropSetting::new16x9(
            50,
            CropLevel::Aggressive,
            frame_width,
            frame_height,
            color_tiles_x,
            color_tiles_y);

        assert_eq!(fifty_crop.full_color_tiles_x(), 16, "fifty_crop: full_color_tiles_x should be 16");
        assert_eq!(fifty_crop.full_color_tiles_y(), 16, "fifty_crop: full_color_tiles_y should be 16");

        let expected_tile_dimensions: (u32, u32) = (120, 67);
        let (tile_width, tile_height) = expected_tile_dimensions;
        assert_eq!(fifty_crop.tile_dimensions(), expected_tile_dimensions, "fifty_crop: expected tile dimensions should be {expected_tile_dimensions:?}");

        let expected_tile_pixels = (tile_width * tile_height) as u32;
        assert_eq!(fifty_crop.total_tile_pixels(), expected_tile_pixels, "fifty_crop: expected tile pixels should be {expected_tile_pixels}");

        let expected_tiles = (fifty_crop.full_color_tiles_x() * fifty_crop.full_color_tiles_y()) as usize;
        assert_eq!(fifty_crop.tiles.len(), expected_tiles, "fifty_crop: should be {expected_tiles} tile entries");

        let first_tile = fifty_crop.tiles.first().unwrap();
        let top_bottom_trim: u32 = 4; // (1080 - (67px * 16 = 1072)) = 8 / 2 = 4
        assert_eq!(first_tile.start_x, 0, "fifty_crop: first tile start_x should be 0");
        assert_eq!(first_tile.end_x, tile_width, "fifty_crop: first tile end_x should be {}", tile_width);
        assert_eq!(first_tile.start_y, top_bottom_trim, "fifty_crop: first tile start_y should be {top_bottom_trim}");
        assert_eq!(first_tile.end_y, tile_height + top_bottom_trim, "fifty_crop: first tile end_y should be {}", tile_height + top_bottom_trim);

        let last_tile = fifty_crop.tiles.last().unwrap();
        assert_eq!(last_tile.start_x, frame_width - tile_width, "fifty_crop: last tile start_x should be {}", frame_width - tile_width);
        assert_eq!(last_tile.end_x, frame_width, "fifty_crop: last tile end_x should be {frame_width}");
        assert_eq!(last_tile.start_y, (frame_height - tile_height) - top_bottom_trim, "fifty_crop: last tile start_y should be {}", (frame_height - tile_height) - top_bottom_trim);
        assert_eq!(last_tile.end_y, frame_height - top_bottom_trim, "fifty_crop: last tile end_y should be {}", frame_height - top_bottom_trim);
    }

    #[test]
    fn extract_crop_tile() {
        let frame_width: u32 = 1920;
        let frame_height: u32 = 1080;

        let color_tiles_x: u8 = 8;
        let color_tiles_y: u8 = 8;

        // 50% crop
        let fifty_crop = CropSetting::new16x9(
            50,
            CropLevel::Aggressive,
            frame_width,
            frame_height,
            color_tiles_x,
            color_tiles_y);

        let top_left = fifty_crop.extract_crop_tile(0, 0);
        assert_eq!(0, top_left.start_x, "top_left: expected x start pos should be 0");
        assert_eq!(960, top_left.end_x, "top_left: xpected x end pos should be 960");

        assert_eq!(4, top_left.start_y, "top_left: expected y start pos should be 4");
        assert_eq!(540, top_left.end_y, "top_left: expected y end pos should be 540");

        let bottom_right = fifty_crop.extract_crop_tile(8, 8);
        assert_eq!(960, bottom_right.start_x, "bottom_right: expected x start pos should be 0");
        assert_eq!(1920, bottom_right.end_x, "bottom_right: xpected x end pos should be 960");

        assert_eq!(540, bottom_right.start_y, "bottom_right: expected y start pos should be 4");
        assert_eq!(1076, bottom_right.end_y, "bottom_right: expected y end pos should be 540");
    }
}