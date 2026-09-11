use std::{path::{Path, PathBuf}, sync::Arc};

use crate::{color_matcher::{FrameMatch, ImageTile}, images::PreviewImage};

pub struct ImageFile {
    pub file_name: String,
    pub full_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: super::ImageType,
    pub preview: Option<PreviewImage>, 
    pub is_chosen: bool,

    pub image_tiles: Option<Arc<Vec<ImageTile>>>,

    pub matched_tiles: Option<Vec<FrameMatch>>,
}

impl ImageFile {
    pub fn new(file_name: &str, full_path: &Path, width: u32, height: u32, format: super::ImageType) -> ImageFile {
        ImageFile {
            file_name: String::from(file_name),
            full_path: PathBuf::from(full_path),
            width,
            height,
            format,
            preview: None,
            is_chosen: false,
            image_tiles: None,
            matched_tiles: None,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ImageType {
    BMP,
    JPEG,
    PNG,
    WEBP,
    TIFF,
}