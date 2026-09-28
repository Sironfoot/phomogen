use std::{sync::{Arc, mpsc::{self, Receiver}}, thread};

use anyhow::Result;

use crate::{app::App, color_matcher::{ColorMatcher, FrameMatch}};

pub struct Response {
    pub num_tiles_x: u8,
    pub num_tiles_y: u8,
    pub frame_match: FrameMatch,
}

pub fn run(app: &mut App) -> Result<Receiver<Response>> {
    let (tx, rc) = mpsc::channel::<Response>();

    let Some(chosen_image) = app.images.iter().find(|i| i.is_chosen) else {
        return Err(anyhow::format_err!("no image selected"));
    };

    let select_tile_shape = &app.selected_tile_shape;
    let Some(tiling_options) = chosen_image.tiling_options.get(select_tile_shape) else {
        return Err(anyhow::format_err!("mosaic tiling layouts not found for selected tile shape"));
    };

    let chosen_tiling_options = tiling_options.iter()
        .filter(|t| t.is_chosen && t.image_tiles.is_some())
        .filter_map(|t| {
            let num_tiles_x = t.num_tiles_x;
            let num_tiles_y = t.num_tiles_y;
            let Some(image_tiles) = &t.image_tiles else { return None; };

            Some((num_tiles_x, num_tiles_y, Arc::clone(image_tiles)))
        })
        .collect::<Vec<_>>();

    if chosen_tiling_options.len() == 0 {
        return Err(anyhow::format_err!("No tile layouts have been chosen"));
    }

    let videos = app.videos.iter()
        .filter(|v| v.is_chosen && v.database.is_some())
        .filter_map(|v| {
            let Some(database) = &v.database else { return None; };
            let filename = v.metadata.file_name.clone();

            Some((filename, Arc::clone(database)))
        })
        .collect::<Vec<_>>();

    if videos.len() == 0 {
        return Err(anyhow::format_err!("No videos have been selected"));
    }

    let allowed_crops = app.allowed_crops().to_vec();
    let max_cores = app.system_info.max_allowed_cores();

    thread::spawn(move || {
        for (num_tiles_x, num_tiles_y, image_tiles) in chosen_tiling_options {
            let mut matcher = ColorMatcher::new(
                num_tiles_x as u32,
                num_tiles_y as u32,
                &allowed_crops);

            matcher.set_thread_count(max_cores);

            for (filename, database) in &videos {
                matcher.add_database(filename, database);
            }

            let rc =  matcher.match_tiles(&image_tiles).unwrap();

            for frame_match in rc {
                tx.send(Response {
                    num_tiles_x,
                    num_tiles_y,
                    frame_match
                }).unwrap();
            }
        }
    });

    Ok(rc)
}