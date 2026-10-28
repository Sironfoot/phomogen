#[derive(Clone, Debug)]
pub enum FrameMatcherAlgorithm {
    Rgb,
    RgbWeighted { red_bias: u8, green_bias: u8, blue_bias: u8 },
    Oklab,
    OklabWeighted { lightness_weight: f32, chroma_weight: f32 },
}