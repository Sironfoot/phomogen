/// Represents x,y coords and height/width dimensions for an area on a video frame
#[derive(Debug, Clone)]
pub struct Tile {
    pub start_x: u32,
    pub end_x: u32,
    pub start_y: u32,
    pub end_y: u32,
}

impl Tile {
    pub fn dimensions(&self) -> (u32, u32) {
        let width = self.end_x - self.start_x;
        let height = self.end_y - self.start_y;

        (width, height)
    }
}