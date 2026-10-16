//! The proportions a crop can be held to: a shape, lying down or standing up.

use anyview_core::PixelSize;

/// The shape a crop is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CropShape {
    /// Any shape.
    #[default]
    Free,
    /// The picture's own proportions.
    Original,
    /// Equal sides.
    Square,
    /// Four to three.
    FourThree,
    /// Sixteen to nine.
    SixteenNine,
}

impl CropShape {
    /// Every shape, in the order the control lists them.
    pub const ALL: [CropShape; 5] = [
        CropShape::Free,
        CropShape::Original,
        CropShape::Square,
        CropShape::FourThree,
        CropShape::SixteenNine,
    ];

    /// What the control calls it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CropShape::Free => "Free",
            CropShape::Original => "Original",
            CropShape::Square => "Square",
            CropShape::FourThree => "4:3",
            CropShape::SixteenNine => "16:9",
        }
    }
}

/// Whether a shape that has a long side lies down or stands up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CropLean {
    /// Wider than it is tall.
    #[default]
    Wide,
    /// Taller than it is wide.
    Tall,
}

impl CropLean {
    /// What the control calls it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CropLean::Wide => "Landscape",
            CropLean::Tall => "Portrait",
        }
    }
}

/// The proportions a crop is held to: a shape, lying down or standing up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CropAspect {
    /// The shape.
    pub shape: CropShape,
    /// Whether it is landscape or portrait. Free and Square have no long side to lean.
    pub lean: CropLean,
}

impl CropAspect {
    /// The width and the height the crop is held to, for a picture shown at `shown`; `None` when
    /// the shape is free.
    #[must_use]
    pub fn ratio(self, shown: PixelSize) -> Option<(u32, u32)> {
        let wide = match self.shape {
            CropShape::Free => return None,
            CropShape::Original => return Some((shown.width.0.max(1), shown.height.0.max(1))),
            CropShape::Square => return Some((1, 1)),
            CropShape::FourThree => (4, 3),
            CropShape::SixteenNine => (16, 9),
        };
        Some(match self.lean {
            CropLean::Wide => wide,
            CropLean::Tall => (wide.1, wide.0),
        })
    }
}
