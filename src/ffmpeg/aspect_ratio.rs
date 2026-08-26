use std::fmt::Display;

#[derive(PartialEq, Clone, Debug)]
pub enum AspectRatio {
    Square1x1,
    Landscape4x3,
    Landscape16x9,
    Landscape21x9,
    Portrait9x16,
    Custom(f64),
}

impl AspectRatio {
    pub fn new(width: u32, height: u32) -> Self {
        let ratio = width as f64 / height as f64;
        let ratio = (ratio * 100.0).round() / 100.0;

        match ratio {
            1.00 => Self::Square1x1,
            1.33 => Self::Landscape4x3,
            1.78 => Self::Landscape16x9,
            2.30..=2.40 => Self::Landscape21x9,
            0.56 => Self::Portrait9x16,
            ratio => Self::Custom(ratio),
        }
    }

    pub fn is_vertical(&self) -> bool {
        match self {
            Self::Portrait9x16 => true,
            Self::Custom(ratio) => *ratio < 1.0,
            _ => false,
        }
    }

    pub fn ratio(&self) -> f64 {
         match self {
            Self::Square1x1 => 1.0,
            Self::Landscape4x3 => 1.3333333333,
            Self::Landscape16x9 => 1.7777777778,
            Self::Landscape21x9 => 2.3333333333,
            Self::Portrait9x16 => 0.5625,
            Self::Custom(ratio) => *ratio,
        }
    }
}

impl Display for AspectRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Square1x1 => write!(f, "1:1"),
            Self::Landscape4x3 => write!(f, "4:3"),
            Self::Landscape16x9 => write!(f, "16:9"),
            Self::Landscape21x9 => write!(f, "21:9"),
            Self::Portrait9x16 => write!(f, "9:16"),
            Self::Custom(ratio) => write!(f, "{ratio}:1"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_4x3() {
        let dimensions: Vec<(u32, u32)> = vec![
            (640, 480),
            (800, 600),
            (1024, 768),
            (1280, 960),
            (1440, 1080),
            (1600, 1200),
            (2048, 1536),
            (3840, 2880),
        ];

        let expected = AspectRatio::Landscape4x3;

        for (width, height) in dimensions {
            let actual = AspectRatio::new(width, height);
            assert_eq!(actual, expected, "{width}x{height} wrong ratio");
            assert!(!actual.is_vertical(), "{width}x{height} is NOT vertical");
        }
    }

     #[test]
    fn check_16x9() {
        let dimensions: Vec<(u32, u32)> = vec![
            (1280, 720),
            (1366, 768),
            (1600, 900),
            (1920, 1080),
            (2560, 1440),
            (3840, 2160),
            (5120, 2880),
            (7680, 4320),
        ];

        let expected = AspectRatio::Landscape16x9;

        for (width, height) in dimensions {
            let actual = AspectRatio::new(width, height);
            assert_eq!(actual, expected, "{width}x{height}");
            assert!(!actual.is_vertical(), "{width}x{height} is NOT vertical");
        }
    }

    #[test]
    fn check_21x9() {
        let dimensions: Vec<(u32, u32)> = vec![
            (2560, 1080),
            (3440, 1440),
            (3840, 1600),
            (5120, 2160),
        ];

        let expected = AspectRatio::Landscape21x9;

        for (width, height) in dimensions {
            let actual = AspectRatio::new(width, height);
            assert_eq!(actual, expected, "{width}x{height}");
            assert!(!actual.is_vertical(), "{width}x{height} is NOT vertical");
        }
    }

    #[test]
    fn check_9x16() {
        let dimensions: Vec<(u32, u32)> = vec![
            (720, 1280),
            (768, 1366),
            (900, 1600),
            (1080, 1920),
            (1440, 2560),
            (2160, 3840),
            (2880, 5120),
            (4320, 7680),
        ];

        let expected = AspectRatio::Portrait9x16;

        for (width, height) in dimensions {
            let actual = AspectRatio::new(width, height);
            assert_eq!(actual, expected, "{width}x{height}");
            assert!(actual.is_vertical(), "{width}x{height} should be vertical");
        }
    }
}