//! EXIF orientation as what it does to the pixels: a mirror, then a clockwise quarter turn.

use crate::pixels::Rgba8;
use anyview_core::{Axis, QuarterTurn};
use image::imageops;

/// Whether the stored picture is mirrored left to right before it is turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mirror {
    /// Not mirrored.
    Unmirrored,
    /// Mirrored left to right.
    Mirrored,
}

/// How the stored pixels must be placed to display upright: mirror them, then turn them
/// clockwise. The eight EXIF orientation values are exactly the eight combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExifOrientation {
    /// Applied first.
    pub mirror: Mirror,
    /// Applied second, clockwise.
    pub turn: QuarterTurn,
}

/// EXIF tag value, mirror and turn, for the values 1 to 8 (EXIF 2.32, tag 0x0112).
const TABLE: &[(u16, Mirror, QuarterTurn)] = &[
    (1, Mirror::Unmirrored, QuarterTurn::None),
    (2, Mirror::Mirrored, QuarterTurn::None),
    (3, Mirror::Unmirrored, QuarterTurn::Half),
    (4, Mirror::Mirrored, QuarterTurn::Half),
    (5, Mirror::Mirrored, QuarterTurn::ThreeQuarter),
    (6, Mirror::Unmirrored, QuarterTurn::Quarter),
    (7, Mirror::Mirrored, QuarterTurn::Quarter),
    (8, Mirror::Unmirrored, QuarterTurn::ThreeQuarter),
];

impl ExifOrientation {
    /// The orientation of a picture that is already upright (tag value 1).
    pub const UPRIGHT: ExifOrientation = ExifOrientation {
        mirror: Mirror::Unmirrored,
        turn: QuarterTurn::None,
    };

    /// The orientation a tag value stands for; `None` outside 1 to 8.
    pub fn from_tag(tag: u16) -> Option<Self> {
        TABLE
            .iter()
            .find(|(value, _, _)| *value == tag)
            .map(|&(_, mirror, turn)| ExifOrientation { mirror, turn })
    }

    /// The tag value, 1 to 8.
    pub fn tag(self) -> u16 {
        TABLE
            .iter()
            .find(|(_, mirror, turn)| *mirror == self.mirror && *turn == self.turn)
            .map_or(1, |(value, _, _)| *value) // the table holds all eight combinations
    }

    /// The orientation of the picture after it is also turned clockwise on screen by `turn`.
    /// Rotating a displayed picture adds to the stored turn and leaves the mirror alone.
    pub fn turned(self, turn: QuarterTurn) -> Self {
        ExifOrientation {
            mirror: self.mirror,
            turn: self.turn.then(turn),
        }
    }

    /// The orientation of the picture after it is also mirrored on screen across `axis`. A
    /// mirror then a turn, mirrored again, is the opposite mirror and the reverse turn; a flip
    /// across the horizontal line is that and a half turn more.
    pub fn flipped(self, axis: Axis) -> Self {
        let mirror = match self.mirror {
            Mirror::Unmirrored => Mirror::Mirrored,
            Mirror::Mirrored => Mirror::Unmirrored,
        };
        let across_vertical = ExifOrientation {
            mirror,
            turn: self.turn.reversed(),
        };
        match axis {
            Axis::Horizontal => across_vertical,
            Axis::Vertical => across_vertical.turned(QuarterTurn::Half),
        }
    }

    /// The pixels placed upright. The size swaps when the turn is a quarter or three quarters.
    pub fn applied(self, picture: &Rgba8) -> Rgba8 {
        let Some(image) = picture.to_image() else {
            return picture.clone(); // a buffer `Rgba8::new` accepted always fits its size
        };
        let mirrored = match self.mirror {
            Mirror::Unmirrored => image,
            Mirror::Mirrored => imageops::flip_horizontal(&image),
        };
        let turned = match self.turn {
            QuarterTurn::None => mirrored,
            QuarterTurn::Quarter => imageops::rotate90(&mirrored),
            QuarterTurn::Half => imageops::rotate180(&mirrored),
            QuarterTurn::ThreeQuarter => imageops::rotate270(&mirrored),
        };
        Rgba8::from_image(turned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{PixelLen, PixelSize};
    use image::metadata::Orientation as ImageOrientation;
    use image::{DynamicImage, RgbaImage};

    /// A 3x2 picture whose six pixels are all different.
    fn marked() -> Rgba8 {
        let bytes = (0u8..6).flat_map(|i| [i * 40, i, 255 - i, 255]).collect();
        let size = PixelSize {
            width: PixelLen(3),
            height: PixelLen(2),
        };
        Rgba8::new(size, bytes).unwrap()
    }

    #[test]
    fn every_tag_places_pixels_as_the_image_crate_does() {
        for tag in 1..=8u16 {
            let ours = ExifOrientation::from_tag(tag).unwrap().applied(&marked());
            let image = RgbaImage::from_raw(3, 2, marked().into_bytes()).unwrap();
            let mut theirs = DynamicImage::ImageRgba8(image);
            let wanted = ImageOrientation::from_exif(u8::try_from(tag).unwrap()).unwrap();
            theirs.apply_orientation(wanted);
            assert_eq!(ours.bytes(), theirs.to_rgba8().as_raw(), "tag {tag}");
        }
    }

    #[test]
    fn flipping_the_orientation_flips_the_pixels_it_places() {
        // The flip of what the tag shows, done on pixels, is what the flipped tag shows.
        let across = |axis| match axis {
            Axis::Horizontal => ExifOrientation {
                mirror: Mirror::Mirrored,
                turn: QuarterTurn::None,
            },
            Axis::Vertical => ExifOrientation {
                mirror: Mirror::Mirrored,
                turn: QuarterTurn::Half,
            },
        };
        for tag in 1..=8u16 {
            for axis in [Axis::Horizontal, Axis::Vertical] {
                let now = ExifOrientation::from_tag(tag).unwrap();
                let shown = now.applied(&marked());
                assert_eq!(
                    now.flipped(axis).applied(&marked()),
                    across(axis).applied(&shown),
                    "tag {tag} flipped {axis:?}"
                );
            }
        }
    }

    #[test]
    fn tags_round_trip_and_only_one_to_eight_exist() {
        for tag in 1..=8u16 {
            assert_eq!(
                ExifOrientation::from_tag(tag).map(ExifOrientation::tag),
                Some(tag)
            );
        }
        assert_eq!(ExifOrientation::from_tag(0), None);
        assert_eq!(ExifOrientation::from_tag(9), None);
    }

    #[test]
    fn turning_a_displayed_picture_composes_with_the_stored_orientation() {
        // name, tag before, turn, tag after
        const CASES: &[(&str, u16, QuarterTurn, u16)] = &[
            ("upright turned a quarter", 1, QuarterTurn::Quarter, 6),
            ("upright turned back", 1, QuarterTurn::ThreeQuarter, 8),
            (
                "quarter turned a quarter is half",
                6,
                QuarterTurn::Quarter,
                3,
            ),
            ("four quarters return", 6, QuarterTurn::ThreeQuarter, 1),
            ("mirrored keeps its mirror", 2, QuarterTurn::Quarter, 7),
            ("nothing", 5, QuarterTurn::None, 5),
        ];
        for (name, before, turn, after) in CASES {
            let got = ExifOrientation::from_tag(*before).unwrap().turned(*turn);
            assert_eq!(got.tag(), *after, "{name}");
        }
    }

    #[test]
    fn turning_pixels_after_applying_matches_applying_the_turned_orientation() {
        for tag in 1..=8u16 {
            for quarters in 0..4u8 {
                let turn = QuarterTurn::from_quarters(quarters);
                let stored = ExifOrientation::from_tag(tag).unwrap();
                let on_screen = stored.applied(&marked());
                let rotated = ExifOrientation {
                    mirror: Mirror::Unmirrored,
                    turn,
                }
                .applied(&on_screen);
                let composed = stored.turned(turn).applied(&marked());
                assert_eq!(rotated, composed, "tag {tag} then {quarters} quarters");
            }
        }
    }
}
