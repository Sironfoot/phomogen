use std::fmt::Display;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
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