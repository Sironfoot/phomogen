use anyhow::Result;
use crate::ffmpeg::AspectRatio;

#[derive(Debug, Clone)]
pub struct CropSetting {
    resize_percentage: f64,
    pub crop_level: CropLevel,

    color_tiles_x: u8,
    color_tiles_y: u8,

    full_color_tiles_x: u32,
    full_color_tiles_y: u32,

    tile_width: u32,
    tile_height: u32,
    tile_pixels: u32,

    pub tiles: Vec<CropTile>,
}

impl CropSetting {
    pub fn resize_percentage(&self) -> f64 {
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

    fn new(resize: f64, crop_level: CropLevel, frame_width: u32, frame_height: u32, color_tiles_x: u8, color_tiles_y: u8) -> Self {
        // example: we're using 9x9 color tiles to extract RGB averages from, and the cropped resize we're using is 50%
        // we therefore devide the entire video frame into 18x18 grid. When it comes time to perform nearest matches for the Mosaic
        // and we need the top-left crop of the frame we grab the top-left 9x9 tiles from that 18x18 grid.
        let full_color_tiles_x = ((100.0 / resize) * color_tiles_x as f64).round() as u32;
        let full_color_tiles_y = ((100.0 / resize) * color_tiles_y as f64).round() as u32;

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
        
        Self {
            resize_percentage: resize,
            crop_level,
            color_tiles_x,
            color_tiles_y,
            full_color_tiles_x,
            full_color_tiles_y,
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
                Ok(Self::get_16x9(frame_width, frame_height, color_tiles_x, color_tiles_y))
            },
            _ => {
                Err(anyhow::format_err!("`{aspect_ratio}` aspect ratio videos aren't supported"))
            }
        }
    }

    fn get_16x9(frame_width: u32, frame_height: u32, color_tiles_x: u8, color_tiles_y: u8) -> Vec<Self> {
        vec![
            Self::new(50.0, CropLevel::Aggressive, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new(66.666, CropLevel::Moderate, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new(80.0, CropLevel::Moderate, frame_width, frame_height, color_tiles_x, color_tiles_y),
            Self::new(100.0, CropLevel::Essential, frame_width, frame_height, color_tiles_x, color_tiles_y),
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
        let full_frame = CropSetting::new(
            100.0,
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
        let fifty_crop = CropSetting::new(
            50.0,
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
}