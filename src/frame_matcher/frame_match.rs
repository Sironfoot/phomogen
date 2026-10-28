#[derive(Debug, Clone)]
pub struct FrameMatch {
    pub tile_index: u32,

    pub video_filename: String,
    pub frame_index: u32,
    
    pub crop_resize: f32,
    pub crop_pos_x: f32,
    pub crop_pos_y: f32,
    pub is_flipped: bool,
}