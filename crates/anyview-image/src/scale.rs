//! Sizes and resampling: fitting a picture into a pixel budget, and the export's resize choices.

use crate::error::ImageError;
use crate::pixels::{PremultipliedRgba8, Rgba8};
use anyview_core::{PeekBudget, PixelArea, PixelLen, PixelSize, Resize};
use image::imageops::{self, FilterType};

/// The most pixels a peek's picture may hold: the pixel budget, and as many pixels as the byte
/// budget has room for at four bytes each. An error when either leaves none.
pub(crate) fn peek_area(budget: &PeekBudget) -> Result<PixelArea, ImageError> {
    match budget.pixels.0.min(budget.bytes.0 / 4) {
        0 => Err(ImageError::NoBudget),
        area => Ok(PixelArea(area)),
    }
}

/// The largest size with the same proportions as `size` that holds at most `max` pixels and is not
/// larger than `size`. Each side is at least one pixel.
pub(crate) fn fit_area(size: PixelSize, max: PixelArea) -> PixelSize {
    let area = size.area().0;
    if area <= max.0 || area == 0 {
        return size;
    }
    let scale = (max.0 as f64 / area as f64).sqrt(); // sizes are far below 2^52
    let mut width = ((f64::from(size.width.0) * scale) as u64).max(1);
    let mut height = ((f64::from(size.height.0) * scale) as u64).max(1);
    // Truncating both sides leaves the area under the budget except at one pixel wide.
    while width * height > max.0 && (width > 1 || height > 1) {
        if width >= height {
            width = (width - 1).max(1);
        } else {
            height = (height - 1).max(1);
        }
    }
    PixelSize {
        width: PixelLen(width as u32), // at most the original width
        height: PixelLen(height as u32),
    }
}

/// The size `resize` makes of `size`, each side at least one pixel.
pub(crate) fn resize_target(size: PixelSize, resize: Resize) -> PixelSize {
    let scaled = |side: u32, numerator: u64, denominator: u64| {
        let value = (u64::from(side) * numerator + denominator / 2) / denominator;
        PixelLen(u32::try_from(value.clamp(1, u64::from(u32::MAX))).unwrap_or(u32::MAX))
    };
    match resize {
        Resize::Original => size,
        Resize::Scaled(permille) => {
            let factor = u64::from(permille.0).max(1);
            PixelSize {
                width: scaled(size.width.0, factor, 1000),
                height: scaled(size.height.0, factor, 1000),
            }
        }
        Resize::LongEdge(edge) => {
            let long = size.width.0.max(size.height.0).max(1);
            let target = u64::from(edge.0).max(1);
            PixelSize {
                width: scaled(size.width.0, target, u64::from(long)),
                height: scaled(size.height.0, target, u64::from(long)),
            }
        }
    }
}

/// `picture` resampled to `size`. Colour is filtered in premultiplied form, so the hidden colour of
/// transparent pixels does not bleed into their neighbours. Downscales use a box-like filter fast
/// enough for a peek; an export resize uses [`Resampling::Sharp`].
pub(crate) fn resampled(picture: &Rgba8, size: PixelSize, filter: Resampling) -> Rgba8 {
    if size == picture.size() {
        return picture.clone();
    }
    let Some(image) = picture.premultiplied().to_image() else {
        return picture.clone();
    };
    let (width, height) = (size.width.0, size.height.0);
    let out = match filter {
        Resampling::Fast => imageops::thumbnail(&image, width, height),
        Resampling::Sharp => imageops::resize(&image, width, height, FilterType::Lanczos3),
    };
    PremultipliedRgba8::from_image(out).unpremultiplied()
}

/// How much work a resample may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resampling {
    /// A box filter, for a thumbnail.
    Fast,
    /// Lanczos, for an export.
    Sharp,
}

/// `picture` resized the way an export asked.
#[must_use]
pub fn resized(picture: &Rgba8, resize: Resize) -> Rgba8 {
    resampled(
        picture,
        resize_target(picture.size(), resize),
        Resampling::Sharp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, Permille};
    use std::time::Duration;

    fn size(width: u32, height: u32) -> PixelSize {
        PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }
    }

    #[test]
    fn a_peek_area_is_the_smaller_of_the_pixel_budget_and_the_byte_room() {
        // name, pixels, bytes, area
        const CASES: &[(&str, u64, u64, Option<u64>)] = &[
            ("pixels bind", 1_000, 1_000_000, Some(1_000)),
            ("bytes bind", 1_000_000, 4_000, Some(1_000)),
            ("bytes round down", 1_000_000, 4_003, Some(1_000)),
            ("no pixels", 0, 1_000_000, None),
            ("under four bytes", 1_000_000, 3, None),
        ];
        for (name, pixels, bytes, want) in CASES {
            let budget = PeekBudget {
                bytes: ByteLen(*bytes),
                pixels: PixelArea(*pixels),
                time: Duration::from_millis(100),
            };
            assert_eq!(peek_area(&budget).ok().map(|a| a.0), *want, "{name}");
        }
    }

    #[test]
    fn fitting_keeps_proportions_never_enlarges_and_stays_inside_the_area() {
        // name, size, max area, fitted
        type Case = (&'static str, (u32, u32), u64, (u32, u32));
        const CASES: &[Case] = &[
            ("already inside", (100, 50), 6_000, (100, 50)),
            ("exactly the budget", (100, 50), 5_000, (100, 50)),
            ("half the area", (100, 50), 2_500, (70, 35)),
            ("tall", (50, 100), 2_500, (35, 70)),
            ("a thin strip", (10_000, 1), 100, (100, 1)),
            ("a single pixel budget", (640, 480), 1, (1, 1)),
            ("empty stays empty", (0, 10), 5, (0, 10)),
        ];
        for (name, (w, h), max, (fw, fh)) in CASES {
            let fitted = fit_area(size(*w, *h), PixelArea(*max));
            assert_eq!(fitted, size(*fw, *fh), "{name}");
            if *w > 0 && *h > 0 {
                assert!(fitted.area().0 <= (*max).max(1), "{name} area");
            }
        }
    }

    #[test]
    fn export_resizes_scale_by_proportion_or_long_edge() {
        // name, source, resize, result
        type Case = (&'static str, (u32, u32), Resize, (u32, u32));
        let cases: &[Case] = &[
            ("original", (400, 300), Resize::Original, (400, 300)),
            (
                "half",
                (400, 300),
                Resize::Scaled(Permille(500)),
                (200, 150),
            ),
            (
                "double",
                (400, 300),
                Resize::Scaled(Permille(2000)),
                (800, 600),
            ),
            ("rounds", (5, 3), Resize::Scaled(Permille(500)), (3, 2)),
            ("never zero", (4, 4), Resize::Scaled(Permille(1)), (1, 1)),
            (
                "long edge wide",
                (400, 300),
                Resize::LongEdge(PixelLen(200)),
                (200, 150),
            ),
            (
                "long edge tall",
                (300, 400),
                Resize::LongEdge(PixelLen(200)),
                (150, 200),
            ),
            (
                "long edge enlarges",
                (40, 30),
                Resize::LongEdge(PixelLen(80)),
                (80, 60),
            ),
        ];
        for (name, (w, h), resize, (rw, rh)) in cases {
            assert_eq!(
                resize_target(size(*w, *h), *resize),
                size(*rw, *rh),
                "{name}"
            );
        }
    }

    #[test]
    fn a_transparent_neighbour_does_not_darken_the_edge_of_a_downscale() {
        // Left pixel opaque white, right pixel transparent black: the average is half-transparent
        // white, not grey.
        let picture = Rgba8::new(size(2, 1), vec![255, 255, 255, 255, 0, 0, 0, 0]).unwrap();
        let one = resampled(&picture, size(1, 1), Resampling::Fast);
        let [r, g, b, a] = <[u8; 4]>::try_from(one.bytes()).unwrap();
        assert!(
            r >= 250 && g >= 250 && b >= 250,
            "colour stays white: {r} {g} {b}"
        );
        assert!((120..=135).contains(&a), "alpha averages to half: {a}");
    }

    #[test]
    fn resizing_changes_the_size_and_leaves_a_matching_size_alone() {
        let picture = Rgba8::new(size(4, 2), vec![200; 32]).unwrap();
        assert_eq!(resized(&picture, Resize::Original), picture);
        let half = resized(&picture, Resize::Scaled(Permille(500)));
        assert_eq!(half.size(), size(2, 1));
    }
}
