use std::{path::Path, time::Instant};

use image::{ImageBuffer, ImageResult, Rgb, RgbImage};

fn main() -> image::ImageResult<()> {
    let timer = Instant::now();

    let output = blend_images_side_by_side(
        Path::new("./videos/blend-test/2x1.png"),
        Some(Path::new("./videos/blend-test/1x1.png")),
        Some(Path::new("./videos/blend-test/2x2.png")),
        Some(Path::new("./videos/blend-test/3x1.png")),
        Some(Path::new("./videos/blend-test/2x0.png")),
        6,
    )?;

    let ellapsed = timer.elapsed().as_millis();
    println!("{ellapsed} ms");

    output.save("./videos/blend-test/0000-output.jpeg")?;

    Ok(())
}

pub fn blend_images_side_by_side(
    image_path: &Path,
    top_image_path: Option<&Path>,
    right_image_path: Option<&Path>,
    bottom_image_path: Option<&Path>,
    left_image_path: Option<&Path>,
    
    blend_width: u32,
) -> ImageResult<RgbImage> {
    let mut center_image = image::open(image_path)?.to_rgb8();

    let top_image = match top_image_path {
        Some(path) => Some(image::open(path)?.to_rgb8()),
        None => None,
    };

    let right_image = match right_image_path {
        Some(path) => Some(image::open(path)?.to_rgb8()),
        None => None,
    };

    let bottom_image = match bottom_image_path {
        Some(path) => Some(image::open(path)?.to_rgb8()),
        None => None,
    };

    let left_image = match left_image_path {
        Some(path) => Some(image::open(path)?.to_rgb8()),
        None => None,
    };

    let (width, height) = center_image.dimensions();

    assert!(
        blend_width > 0 && blend_width <= width,
        "Invalid blend width"
    );

    let sample_width: u32 = 10;
    let sample_height: u32 = 10;

    if let Some(top_image) = &top_image {
        for x in 0..width {
            let top_average = average_top_edge(&top_image, x, sample_height);
            let center_average = average_bottom_edge(&center_image, x, sample_height);

            let seam_colour = Rgb([
                ((top_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                ((top_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                ((top_average[2] as u16 + center_average[2] as u16) / 2) as u8,
            ]);

            for i in 0..blend_width {
                // 0.0 at the outside of the blend area,
                // 1.0 at the actual seam.
                let t = (i + 1) as f32 / blend_width as f32;

                // Smoothstep:
                // gives a softer transition than a simple linear interpolation.
                let t = t * t * (3.0 - 2.0 * t);

                // ---------------------------------------------------------
                // Top edge of center image
                //
                // Reverse i so that blending is strongest at x = 0.
                // ---------------------------------------------------------

                let y2 = blend_width - 1 - i;

                let original = *center_image.get_pixel(x, y2);
                let blended = blend_pixel(original, seam_colour, t);
                center_image.put_pixel(x, y2, blended);
            }
        }
    }

    if let Some(right_image) = &right_image {
        for y in 0..height {
            let left_average = average_right_edge(&center_image, y, sample_width);
            let right_average = average_left_edge(&right_image, y, sample_width);

            let seam_colour = Rgb([
                ((left_average[0] as u16 + right_average[0] as u16) / 2) as u8,
                ((left_average[1] as u16 + right_average[1] as u16) / 2) as u8,
                ((left_average[2] as u16 + right_average[2] as u16) / 2) as u8,
            ]);

            for i in 0..blend_width {
                // 0.0 at the outside of the blend area,
                // 1.0 at the actual seam.
                let t = (i + 1) as f32 / blend_width as f32;

                // Smoothstep:
                // gives a softer transition than a simple linear interpolation.
                let t = t * t * (3.0 - 2.0 * t);

                // ---------------------------------------------------------
                // Right edge of center image
                // ---------------------------------------------------------

                let x1 = width - blend_width + i;

                let original = *center_image.get_pixel(x1, y);
                let blended = blend_pixel(original, seam_colour, t);
                center_image.put_pixel(x1, y, blended);
            }
        }
    }

    if let Some(bottom_image) = &bottom_image {
        for x in 0..width {
            let bottom_average = average_top_edge(bottom_image, x, sample_height);
            let center_average = average_bottom_edge(&center_image, x, sample_height);

            let seam_colour = Rgb([
                ((bottom_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                ((bottom_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                ((bottom_average[2] as u16 + center_average[2] as u16) / 2) as u8,
            ]);

            for i in 0..blend_width {
                // 0.0 at the outside of the blend area,
                // 1.0 at the actual seam.
                let t = (i + 1) as f32 / blend_width as f32;

                // Smoothstep:
                // gives a softer transition than a simple linear interpolation.
                let t = t * t * (3.0 - 2.0 * t);

                // ---------------------------------------------------------
                // Right edge of center image
                // ---------------------------------------------------------

                let y1 = height - blend_width + i;

                let original = *center_image.get_pixel(x, y1);
                let blended = blend_pixel(original, seam_colour, t);
                center_image.put_pixel(x, y1, blended);
            }
        }
    }

    if let Some(left_image) = &left_image {
        for y in 0..height {
            let left_average = average_right_edge(&left_image, y, sample_width);
            let center_average = average_left_edge(&center_image, y, sample_width);

            let seam_colour = Rgb([
                ((left_average[0] as u16 + center_average[0] as u16) / 2) as u8,
                ((left_average[1] as u16 + center_average[1] as u16) / 2) as u8,
                ((left_average[2] as u16 + center_average[2] as u16) / 2) as u8,
            ]);

            for i in 0..blend_width {
                // 0.0 at the outside of the blend area,
                // 1.0 at the actual seam.
                let t = (i + 1) as f32 / blend_width as f32;

                // Smoothstep:
                // gives a softer transition than a simple linear interpolation.
                let t = t * t * (3.0 - 2.0 * t);

                // ---------------------------------------------------------
                // Left edge of center image
                //
                // Reverse i so that blending is strongest at x = 0.
                // ---------------------------------------------------------

                let x2 = blend_width - 1 - i;

                let original = *center_image.get_pixel(x2, y);
                let blended = blend_pixel(original, seam_colour, t);
                center_image.put_pixel(x2, y, blended);
            }
        }
    }

    Ok(ImageBuffer::from(center_image))

    // let mut output: RgbImage = ImageBuffer::new(width * 3, height * 3);

    // // Copy center image
    // for y in 0..height {
    //     for x in 0..width {
    //         output.put_pixel(x + width, y + height, *center_image.get_pixel(x, y));
    //     }
    // }

    // // Copy top
    // if let Some(top_image) = top_image {
    //     for y in 0..height {
    //         for x in 0..width {
    //             output.put_pixel(width + x, y, *top_image.get_pixel(x, y));
    //         }
    //     }
    // }

    // // copy right
    // if let Some(right_image) = right_image {
    //     for y in 0..height {
    //         for x in 0..width {
    //             output.put_pixel((width * 2) + x, y + height, *right_image.get_pixel(x, y));
    //         }
    //     }
    // }

    // // copy bottom
    // if let Some(bottom_image) = bottom_image {
    //     for y in 0..height {
    //         for x in 0..width {
    //             output.put_pixel(width + x, (height * 2) + y, *bottom_image.get_pixel(x, y));
    //         }
    //     }
    // }

    // // copy left
    // if let Some(left_image) = left_image {
    //     for y in 0..height {
    //         for x in 0..width {
    //             output.put_pixel(x, y + height, *left_image.get_pixel(x, y));
    //         }
    //     }
    // }

    // Ok(output)
}

fn blend_pixel(
    original: Rgb<u8>,
    target: Rgb<u8>,
    amount: f32,
) -> Rgb<u8> {
    Rgb([
        lerp(original[0], target[0], amount),
        lerp(original[1], target[1], amount),
        lerp(original[2], target[2], amount),
    ])
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn average_right_edge(image: &RgbImage, y: u32, sample_width: u32) -> Rgb<u8> {
    let width = image.width();

    let start_x = width - sample_width;

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for x in start_x..width {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_width) as u8,
        (g / sample_width) as u8,
        (b / sample_width) as u8,
    ])
}

fn average_bottom_edge(image: &RgbImage, x: u32, sample_height: u32) -> Rgb<u8> {
    let height = image.height();

    let start_y = height - sample_height;

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for y in start_y..height {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_height) as u8,
        (g / sample_height) as u8,
        (b / sample_height) as u8,
    ])
}

fn average_top_edge(image: &RgbImage, x: u32, sample_height: u32) -> Rgb<u8> {
    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for y in 0..sample_height {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_height) as u8,
        (g / sample_height) as u8,
        (b / sample_height) as u8,
    ])
}

fn average_left_edge(image: &RgbImage, y: u32, sample_width: u32) -> Rgb<u8> {
    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for x in 0..sample_width {
        let pixel = image.get_pixel(x, y);

        r += pixel[0] as u32;
        g += pixel[1] as u32;
        b += pixel[2] as u32;
    }

    Rgb([
        (r / sample_width) as u8,
        (g / sample_width) as u8,
        (b / sample_width) as u8,
    ])
}