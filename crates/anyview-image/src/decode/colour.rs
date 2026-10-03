//! How an image stores its colour, for the facts: channels and bits per channel.

use image::ColorType;

/// The channels an image has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColourModel {
    /// One grey channel.
    Grey,
    /// Grey and alpha.
    GreyAlpha,
    /// Red, green and blue.
    Rgb,
    /// Red, green, blue and alpha.
    Rgba,
}

/// The colour channels and depth of the file as stored, before the viewer widens it to RGBA8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColourInfo {
    /// Which channels.
    pub model: ColourModel,
    /// Bits in each channel.
    pub bits: u8,
}

impl ColourInfo {
    /// The colour of `channels` channels of `bits` bits: 1 is grey, 2 grey with alpha, 3 RGB and
    /// anything more RGBA.
    pub(crate) fn of_channels(channels: u8, bits: u8) -> Self {
        let model = match channels {
            0 | 1 => ColourModel::Grey,
            2 => ColourModel::GreyAlpha,
            3 => ColourModel::Rgb,
            _ => ColourModel::Rgba,
        };
        ColourInfo { model, bits }
    }

    pub(crate) fn of_color_type(color: ColorType) -> Self {
        let channels = color.channel_count().max(1);
        let bits = color.bits_per_pixel() / u16::from(channels);
        ColourInfo::of_channels(channels, u8::try_from(bits).unwrap_or(u8::MAX))
    }

    /// `RGB, 8-bit`.
    pub fn text(self) -> String {
        let model = match self.model {
            ColourModel::Grey => "Grey",
            ColourModel::GreyAlpha => "Grey and alpha",
            ColourModel::Rgb => "RGB",
            ColourModel::Rgba => "RGBA",
        };
        format!("{model}, {}-bit", self.bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_types_read_as_channels_and_depth() {
        // name, colour type, text
        const CASES: &[(&str, ColorType, &str)] = &[
            ("grey", ColorType::L8, "Grey, 8-bit"),
            ("grey alpha", ColorType::La8, "Grey and alpha, 8-bit"),
            ("rgb", ColorType::Rgb8, "RGB, 8-bit"),
            ("rgba", ColorType::Rgba8, "RGBA, 8-bit"),
            ("deep grey", ColorType::L16, "Grey, 16-bit"),
            ("deep rgba", ColorType::Rgba16, "RGBA, 16-bit"),
            ("float rgb", ColorType::Rgb32F, "RGB, 32-bit"),
        ];
        for (name, color, text) in CASES {
            assert_eq!(ColourInfo::of_color_type(*color).text(), *text, "{name}");
        }
    }
}
