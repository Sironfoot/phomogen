use std::sync::mpsc::Receiver;

use crate::{app::App, color_matcher::{ColorMatcher, FrameMatch}};

pub fn run(app: &mut App) -> Receiver<FrameMatch> {
    let chosen_image = app.images.iter()
        .find(|i| i.is_chosen && i.image_tiles.is_some());

    if let Some(chosen_image) = chosen_image {
        let mut matcher = ColorMatcher::new(app.mosaic_tiles_x, app.mosaic_tiles_y, app.allowed_crops());

        matcher.set_thread_count(app.system_info.max_allowed_cores());

        let videos = app.videos.iter()
            .filter(|v| v.is_chosen && v.database.is_some());

        for video in videos {
            if let Some(database) = &video.database {
                matcher.add_database(&video.metadata.file_name, database);
            }
        }

        return matcher.match_tiles(chosen_image).unwrap();
    }

    panic!("No image");
}