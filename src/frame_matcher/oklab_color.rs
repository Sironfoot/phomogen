use crate::frame_matcher::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OklabColor {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl OklabColor {
    pub fn from_srgb(red: u8, green: u8, blue: u8) -> Self {
        let red = Self::srgb_channel_to_linear(red);
        let green = Self::srgb_channel_to_linear(green);
        let blue = Self::srgb_channel_to_linear(blue);

         // Linear RGB → LMS.
        let l = 0.412_221_46 * red
            + 0.536_332_55 * green
            + 0.051_445_995 * blue;

        let m = 0.211_903_5 * red
            + 0.680_699_5 * green
            + 0.107_396_96 * blue;

        let s = 0.088_302_46 * red
            + 0.281_718_85 * green
            + 0.629_978_7 * blue;

        let l_root = l.cbrt();
        let m_root = m.cbrt();
        let s_root = s.cbrt();

        // Nonlinear LMS → Oklab.
        Self {
            l: 0.210_454_26 * l_root
                + 0.793_617_8 * m_root
                - 0.004_072_047 * s_root,

            a: 1.977_998_5 * l_root
                - 2.428_592_2 * m_root
                + 0.450_593_7 * s_root,

            b: 0.025_904_037 * l_root
                + 0.782_771_77 * m_root
                - 0.808_675_77 * s_root,
        }
    }

    pub fn from_color(color: &Color) -> Self {
        Self::from_srgb(color.r, color.g, color.b)
    }

    pub fn distance(&self, other: &Self) -> f32 {
        self.distance_weighted(other, 1.0, 1.0)
    }

    pub fn distance_weighted(&self, other: &Self, lightness_weight: f32, chroma_weight: f32) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;

        (lightness_weight * dl * dl) + (chroma_weight * (da * da + db * db))
    }

    fn srgb_channel_to_linear(channel: u8) -> f32 {
        let value = channel as f32 / 255.0;

        if value <= 0.04045 {
            return value / 12.92;
        }

        ((value + 0.055) / 1.055).powf(2.4)
    }
}