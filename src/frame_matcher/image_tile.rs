use crate::frame_matcher::{Color, OklabColor};

#[derive(Debug, Clone)]
pub struct ImageTile {
    pub colors: Vec<Color>,
    pub oklab_colors: Vec<OklabColor>,
}