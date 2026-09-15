use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum PrintSize {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    PostageStamp,
}

impl PrintSize {
    pub fn measurements_portrait(&self) -> (u32, u32) {
        match self {
            Self::A0 => (841, 1189),
            Self::A1 => (594, 841),
            Self::A2 => (420, 594),
            Self::A3 => (297, 420),
            Self::A4 => (210, 297),
            Self::A5 => (148, 210),
            Self::A6 => (105, 148),
            Self::PostageStamp => (20, 28),
        }
    }

    pub fn measurements_landscape(&self) -> (u32, u32) {
        let (width, height) = self.measurements_portrait();
        (height, width)
    }

    pub fn from_mosaic_tile_layout(num_x: u8, num_y: u8) -> Self {
        let longest_side_tiles = num_x.max(num_y);

        match longest_side_tiles {
            0..2 => Self::PostageStamp,
            2..8 => Self::A6,
            8..15 => Self::A5,
            15..25 => Self::A4,
            25..42 => Self::A3,
            42..50 => Self::A2,
            50..60 => Self::A1,
            60.. => Self::A0,
        }
    }

    pub fn measurements_for_contained_image(&self, width: u32, height: u32) -> (u32, u32) {
        let is_landscape = width > height;
        let ratio = width as f64 / height as f64;

        let (mut mm_width, mut mm_height) =
            if is_landscape { self.measurements_landscape() }
            else { self.measurements_portrait() };

        let print_ratio = mm_width as f64 / mm_height as f64;

        if print_ratio > ratio {
            mm_width = (mm_height as f64 * ratio).round() as u32;
        }
        else {
            mm_height = (mm_width as f64 / ratio).round() as u32;
        }

        (mm_width, mm_height)
    }
}

impl Display for PrintSize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurements_for_contained_image() {
        // landscape dimensions only
        let tests: Vec<(PrintSize, u32, u32, u32, u32)> = vec![
            (PrintSize::A0, 1920, 1080, 1189, 669),
            (PrintSize::A0, 2500, 2268, 927, 841),
        ];

        for (print_size, width, height, expected_mm_width, expected_mm_height) in tests {
            let (mm_width, mm_height) = print_size.measurements_for_contained_image(width, height);
            assert_eq!(expected_mm_width, mm_width, "width not correct with {print_size}");
            assert_eq!(expected_mm_height, mm_height, "height not correct for {print_size}");

            // try portrait
            let (width, height) = (height, width);
            let (expected_mm_width, expected_mm_height) = (expected_mm_height, expected_mm_width);

            let (mm_width, mm_height) = print_size.measurements_for_contained_image(width, height);
            assert_eq!(expected_mm_width, mm_width, "width not correct with {print_size} (portrait)");
            assert_eq!(expected_mm_height, mm_height, "height not correct for {print_size} (portrait)");
        }
    }
}