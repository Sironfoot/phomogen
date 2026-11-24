#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropLevel {
    Essential = 1,
    Moderate = 2,
    Aggressive = 3,
}

impl TryFrom<u8> for CropLevel {
    type Error = u8;

    fn try_from(value: u8) -> std::prelude::v1::Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Essential),
            2 => Ok(Self::Moderate),
            3 => Ok(Self::Aggressive),
            invalid => Err(invalid),
        }
    }
}