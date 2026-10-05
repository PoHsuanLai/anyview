//! Placing raw pixels: the mirror and quarter turn of an orientation applied to samples of any
//! width, so a 16-bit or grey picture is moved without being converted to RGBA8 first.

use crate::orientation::{ExifOrientation, Mirror};
use anyview_core::QuarterTurn;

/// A grid of pixels, `unit` bytes each, row by row.
pub(super) struct Grid {
    /// Pixels per row.
    pub width: u32,
    /// Rows.
    pub height: u32,
    /// Bytes per pixel.
    pub unit: usize,
    /// `width * height * unit` bytes.
    pub bytes: Vec<u8>,
}

impl Grid {
    /// The grid with `orientation` applied: mirrored left to right, then turned clockwise. The
    /// width and height swap for a quarter or three-quarter turn. A grid whose buffer is shorter
    /// than its size says is returned as it was.
    pub(super) fn placed(self, orientation: ExifOrientation) -> Grid {
        let (w, h, unit) = (self.width as usize, self.height as usize, self.unit);
        if self.bytes.len() < w * h * unit || (w == 0 || h == 0) {
            return self;
        }
        let swaps = matches!(
            orientation.turn,
            QuarterTurn::Quarter | QuarterTurn::ThreeQuarter
        );
        let (out_w, out_h) = if swaps { (h, w) } else { (w, h) };
        let mut out = vec![0u8; w * h * unit];
        for y in 0..h {
            for x in 0..w {
                let mx = match orientation.mirror {
                    Mirror::Unmirrored => x,
                    Mirror::Mirrored => w - 1 - x,
                };
                let (tx, ty) = match orientation.turn {
                    QuarterTurn::None => (x, y),
                    QuarterTurn::Quarter => (h - 1 - y, x),
                    QuarterTurn::Half => (w - 1 - x, h - 1 - y),
                    QuarterTurn::ThreeQuarter => (y, w - 1 - x),
                };
                let from = (y * w + mx) * unit;
                let to = (ty * out_w + tx) * unit;
                out[to..to + unit].copy_from_slice(&self.bytes[from..from + unit]);
            }
        }
        Grid {
            width: out_w as u32, // at most the old width or height
            height: out_h as u32,
            unit,
            bytes: out,
        }
    }
}

/// Whether `orientation` swaps width and height.
pub(super) fn swaps_axes(orientation: ExifOrientation) -> bool {
    matches!(
        orientation.turn,
        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_orientation_places_samples_as_it_places_rgba() {
        use crate::pixels::Rgba8;
        use anyview_core::{PixelLen, PixelSize};
        let (w, h) = (3u32, 2u32);
        let bytes: Vec<u8> = (0..w * h * 4).map(|i| i as u8).collect();
        let picture = Rgba8::new(
            PixelSize {
                width: PixelLen(w),
                height: PixelLen(h),
            },
            bytes.clone(),
        )
        .unwrap();
        for tag in 1..=8 {
            let o = ExifOrientation::from_tag(tag).unwrap();
            let want = o.applied(&picture);
            let got = Grid {
                width: w,
                height: h,
                unit: 4,
                bytes: bytes.clone(),
            }
            .placed(o);
            assert_eq!(got.bytes, want.bytes(), "tag {tag}");
            assert_eq!(got.width, want.size().width.0, "tag {tag}");
        }
    }
}
