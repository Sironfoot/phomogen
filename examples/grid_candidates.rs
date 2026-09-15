use std::{fmt::Display, time::Instant};

const MAX_TILES_PER_AXIS: u8 = 80;

fn main() {
    let width: u32 = 5000;
    let height: u32 = 5000;
    let tile_shape = TileShape::Landscape16x9;
    const MAX_CROP_PERCENTAGE: f64 = 0.9;

    let timer = Instant::now();

    let candidates = tile_candidates(width, height, MAX_CROP_PERCENTAGE, &tile_shape);

    println!("For a {width}x{height} image ({}):\n", timer.elapsed().as_micros());

    for candidate in &candidates {
        println!(
            "{}x{} ({:.2}% crop, {}x{}) - Print: {:?}",
            candidate.num_tiles_x,
            candidate.num_tiles_y,
            candidate.crop_percentage,
            candidate.cropped_width,
            candidate.cropped_height,
            candidate.ideal_print_size,
        );
    }

    let candidates = narrow_down_candidates(&candidates, width, height, &tile_shape);

    println!("\nNarrowed candidates :\n");

    for candidate in &candidates {
        println!(
            "{}x{} ({:.2}% crop, {}x{}) - Print: {:?}",
            candidate.num_tiles_x,
            candidate.num_tiles_y,
            candidate.crop_percentage,
            candidate.cropped_width,
            candidate.cropped_height,
            candidate.ideal_print_size,
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileCandidate {
    pub num_tiles_x: u8,
    pub num_tiles_y: u8,
    pub crop_percentage: f64,
    pub cropped_width: u32,
    pub cropped_height: u32,
    pub ideal_print_size: PrintSizes,
}

pub fn narrow_down_candidates(candidates: &[TileCandidate], image_width: u32, image_height: u32, tile_shape: &TileShape) -> Vec<TileCandidate> {
    const MAX_CANDIDATES: usize = 20;

    let total_candidates = candidates.len();
    
    if total_candidates < MAX_CANDIDATES {
        return candidates.to_vec();
    }

    let mut narrowed_candidates: Vec<TileCandidate> = Vec::with_capacity(MAX_CANDIDATES);

    // image shape perfectly matches tile shape, expecting 1x1, 2x2, 3x3, 4x4.....80x80
    let image_aspect_ratio = image_width as f64 / image_height as f64;
    if image_aspect_ratio == tile_shape.aspect_ratio() {
        for candidate in candidates.iter() {

            // output 1x1, 5x5, 10x10, 15x15, 20x20, 25x25, 30x30....80x80
            match candidate.num_tiles_x {
                1 => narrowed_candidates.push(candidate.clone()),
                x if x % 5 == 0 =>  narrowed_candidates.push(candidate.clone()),
                _ => {},
            }
        }

        return narrowed_candidates;
    }

    let skip_count = (total_candidates as f64 / MAX_CANDIDATES as f64).round() as usize;

    // all tile arrangements fit perfectly
    let all_candidates_have_no_crops = candidates.iter().all(|c| c.crop_percentage == 0.0);
    if all_candidates_have_no_crops {
        for (i, candidate) in candidates.iter().enumerate() {
            if i % skip_count == 0 {
                narrowed_candidates.push(candidate.clone());
            }
        }
    }
    else {
        for (i, candidate) in candidates.iter().enumerate() {
            // always include first
            if i == 0 {
                narrowed_candidates.push(candidate.clone());
                continue;
            }

            if i % skip_count == 0 {
                let prev_candidate = &candidates[i-1];
                let next_candidate = candidates.get(i + 1);

                let mut compare: Vec<&TileCandidate> = vec![prev_candidate, candidate];
                if let Some(next_candidate) = next_candidate {
                    compare.push(next_candidate);
                }

                let with_smallest_cropping = compare.iter()
                    .min_by_key(|c| (c.crop_percentage * 1000.0).round() as u32);
                if let Some(with_smallest_cropping) = with_smallest_cropping {
                    if !narrowed_candidates.contains(*with_smallest_cropping) {
                        narrowed_candidates.push(with_smallest_cropping.to_owned().clone());
                    }
                    else {
                        narrowed_candidates.push(candidate.clone());
                    }
                }
            }
        }
    }

    narrowed_candidates
}

/// Finds every grid requiring no more than `max_crop_percentage` cropping.
///
/// A grid wider than the image crops the top and bottom. A narrower grid crops
/// the left and right. The returned crop value is a percentage from 0 to 100.
pub fn tile_candidates(image_width: u32, image_height: u32, max_crop_percentage: f64, tile_shape: &TileShape) -> Vec<TileCandidate> {
    if image_width == 0
        || image_height == 0
        || !max_crop_percentage.is_finite()
        || !(0.0..=100.0).contains(&max_crop_percentage)
    {
        return Vec::new();
    }

    let (tile_aspect_width, tile_aspect_height) = tile_shape.aspect_dimensions();

    let image_area = image_width * image_height;

    let mut candidates = Vec::new();

    for tiles_x in 1..=MAX_TILES_PER_AXIS {
        for tiles_y in 1..=MAX_TILES_PER_AXIS {
            // Compare:
            //
            //     16 * tiles_x       image_width
            //     ------------   vs  -----------
            //      9 * tiles_y       image_height
            //
            // without floating-point arithmetic.
            let grid_width = tile_aspect_width as u32 * tiles_x as u32;
            let grid_height = tile_aspect_height as u32 * tiles_y as u32;

            let grid_cross = grid_width * image_height;
            let image_cross = grid_height * image_width;

            let (cropped_width, cropped_height) = if grid_cross > image_cross {
                // The grid is wider, so preserve the width and crop vertically.
                let cropped_height = divide_round(image_width * grid_height, grid_width).min(image_height);

                (image_width, cropped_height)
            }
            else if grid_cross < image_cross {
                // The grid is narrower, so preserve the height and crop horizontally.
                let cropped_width = divide_round(image_height * grid_width, grid_height).min(image_width);

                (cropped_width, image_height)
            }
            else {
                (image_width, image_height)
            };

            let cropped_area = cropped_width * cropped_height;
            let crop_percentage = image_area.abs_diff(cropped_area) as f64 / image_area as f64 * 100.0;

            if crop_percentage <= max_crop_percentage {
                let longest_side_tiles = tiles_x.max(tiles_y);

                let ideal_print_size = match longest_side_tiles {
                    0..2 => PrintSizes::PostageStamp,
                    2..8 => PrintSizes::A6,
                    8..15 => PrintSizes::A5,
                    15..25 => PrintSizes::A4,
                    25..42 => PrintSizes::A3,
                    42..50 => PrintSizes::A2,
                    50..60 => PrintSizes::A1,
                    60.. => PrintSizes::A0,
                };

                candidates.push(TileCandidate {
                    num_tiles_x: tiles_x,
                    num_tiles_y: tiles_y,
                    crop_percentage,
                    cropped_width,
                    cropped_height,
                    ideal_print_size
                });
            }
        }
    }

    candidates.sort_by_key(|candidate| candidate.num_tiles_x);

    candidates
}

fn divide_round(numerator: u32, denominator: u32) -> u32 {
    (numerator + denominator / 2) / denominator
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrintSizes {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    PostageStamp,
}

impl PrintSizes {
    pub fn get_measurements(&self) -> (u32, u32) {
        match self {
            Self::A0 => (1189, 841),
            Self::A1 => (841, 594),
            Self::A2 => (594, 420),
            Self::A3 => (420, 297),
            Self::A4 => (297, 210),
            Self::A5 => (210, 148),
            Self::A6 => (148, 105),
            Self::PostageStamp => (25, 20),
        }
    }
}

impl Display for PrintSizes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A0 => write!(f, "A0"),
            Self::A1 => write!(f, "A1"),
            Self::A2 => write!(f, "A2"),
            Self::A3 => write!(f, "A3"),
            Self::A4 => write!(f, "A4"),
            Self::A5 => write!(f, "A5"),
            Self::A6 => write!(f, "A6"),
            Self::PostageStamp => write!(f, "Postage Stamp"),
        }
    }
}

pub enum TileShape {
    Landscape16x9,
    Square,
    Portrait9x16,
}

impl TileShape {
    pub fn aspect_dimensions(&self) -> (u8, u8) {
        match self {
            Self::Landscape16x9 => (16, 9),
            Self::Square => (1, 1),
            Self::Portrait9x16 => (9, 16),
        }
    }

    pub fn aspect_ratio(&self) -> f64 {
        let (dim_x, dim_y) = self.aspect_dimensions();
        dim_x as f64 / dim_y as f64
    }
}

impl Display for TileShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Landscape16x9 => write!(f, "Landscape (16:9)"),
            Self::Square => write!(f, "Square"),
            Self::Portrait9x16 => write!(f, "Portrait (9:16)"),
        }
    }
}