use std::{fs, sync::mpsc::{self, Receiver}, thread};

use image::{ImageFormat, ImageReader};

use crate::app::{App, ImageFile, ImageType};

pub fn run(app: &App) -> Receiver<Vec<ImageFile>> {
    let (tx, rc) = mpsc::channel::<Vec<ImageFile>>();
    let working_dir = app.working_dir.clone();

    thread::spawn(move || {
        const IMAGE_EXTENSIONS: &[&str] = &[
            "jpg", "jpeg", "png", "webp", "bmp", "tiff"
        ];

        let mut image_files: Vec<String> = vec![];

        let entries = fs::read_dir(&working_dir).unwrap();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap().display();
                let ext = match path.extension() {
                    Some(ext) => Some(ext.display().to_string()),
                    None => None,
                };
                
                if let Some(file_ext) = ext {
                    let allowed = IMAGE_EXTENSIONS.iter()
                        .any(|ext| ext.eq_ignore_ascii_case(&file_ext));

                    if allowed {
                        image_files.push(file_name.to_string());
                    }
                }
            }
        }

        let mut images: Vec<ImageFile> = vec![];

        for image_file in image_files {
            let full_path = working_dir.join(image_file.clone());

            let Ok(image) = ImageReader::open(full_path.clone()) else { continue; };
            let Ok(image) = image.with_guessed_format() else { continue; };
            let Some(image_format) = image.format() else { continue; };
            let Ok((width, height)) = image.into_dimensions() else { continue; };

            let format = match image_format {
                ImageFormat::Bmp => ImageType::BMP,
                ImageFormat::Jpeg => ImageType::JPEG,
                ImageFormat::Png => ImageType::PNG,
                ImageFormat::WebP => ImageType::WEBP,
                ImageFormat::Tiff => ImageType::TIFF,
                _ => { continue; }
            };

            images.push(ImageFile::new(
                image_file.as_str(),
                full_path.as_path(),
                width,
                height,
                format,
            ));
            
        }

        images.sort_by_cached_key(|i| i.file_name.to_lowercase());

        tx.send(images).unwrap();
    });

    rc
}