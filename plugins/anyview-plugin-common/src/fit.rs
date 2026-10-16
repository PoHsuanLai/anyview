//! How big a picture may be shown, and the size that keeps its shape within that.

/// What the picture may measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// No side longer than this many pixels.
    Edge(u32),
    /// No more than this many pixels in all.
    Area(u64),
}

/// A picture as straight RGBA8 rows from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// The size within `fit` that keeps the shape of `size` and never enlarges it.
pub fn fitted(size: (u32, u32), fit: Fit) -> (u32, u32) {
    let (w, h) = (u64::from(size.0), u64::from(size.1));
    let scaled = |scale: f64| {
        let side = |n: u64| ((n as f64 * scale).floor() as u64).max(1);
        (side(w), side(h))
    };
    let (nw, nh) = match fit {
        Fit::Edge(edge) => {
            let longer = w.max(h);
            if longer <= u64::from(edge) {
                (w, h)
            } else {
                scaled(f64::from(edge) / longer as f64)
            }
        }
        Fit::Area(area) => {
            if w * h <= area {
                (w, h)
            } else {
                let (mut nw, mut nh) = scaled((area as f64 / (w * h) as f64).sqrt());
                while nw * nh > area && (nw > 1 || nh > 1) {
                    if nw > 1 {
                        nw -= 1;
                    } else {
                        nh -= 1;
                    }
                }
                (nw, nh)
            }
        }
    };
    (
        u32::try_from(nw).unwrap_or(u32::MAX),
        u32::try_from(nh).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_fitted_without_being_enlarged() {
        type Size = (u32, u32);
        const CASES: &[(&str, Size, Fit, Size)] = &[
            ("small stays", (64, 48), Fit::Edge(256), (64, 48)),
            ("wide", (1920, 1080), Fit::Edge(256), (256, 144)),
            ("tall", (1080, 1920), Fit::Edge(256), (144, 256)),
            ("area allows", (64, 48), Fit::Area(10_000), (64, 48)),
            (
                "area shrinks",
                (6000, 4000),
                Fit::Area(1_000_000),
                (1224, 816),
            ),
            ("a sliver", (10_000, 1), Fit::Area(100), (100, 1)),
        ];
        for (name, size, fit, want) in CASES {
            let got = fitted(*size, *fit);
            assert_eq!(got, *want, "{name}");
            if let Fit::Area(area) = fit {
                assert!(u64::from(got.0) * u64::from(got.1) <= *area, "{name}");
            }
        }
    }
}
