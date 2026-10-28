use crate::app::FrameCrop;

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