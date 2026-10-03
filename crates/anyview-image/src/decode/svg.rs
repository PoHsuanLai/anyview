//! SVG through `resvg`: rendered to straight RGBA8 at a size the caller picks.

use super::{MAX_DECODE_AREA, SVG_LONG_EDGE_MAX, SVG_LONG_EDGE_MIN};
use crate::error::ImageError;
use crate::pixels::{PremultipliedRgba8, Rgba8};
use crate::scale::fit_area;
use anyview_core::{PixelLen, PixelSize};
use resvg::{tiny_skia, usvg};

fn decode_error(error: impl std::fmt::Display) -> ImageError {
    ImageError::Decode {
        reason: error.to_string(),
    }
}

/// A parsed SVG document.
pub(crate) struct Svg(usvg::Tree);

impl Svg {
    /// The document in `bytes`. Text is not drawn (no font database is loaded) and external files
    /// are not read: only what the SVG itself contains is shown.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Svg, ImageError> {
        usvg::Tree::from_data(bytes, &usvg::Options::default())
            .map(Svg)
            .map_err(decode_error)
    }

    /// The size the document declares, rounded up to whole pixels and at least one.
    pub(crate) fn intrinsic(&self) -> PixelSize {
        let size = self.0.size();
        let whole = |length: f32| PixelLen((length.ceil() as u32).max(1)); // saturating cast
        PixelSize {
            width: whole(size.width()),
            height: whole(size.height()),
        }
    }

    /// The document drawn at `size`, stretched to fit it.
    pub(crate) fn render(&self, size: PixelSize) -> Result<Rgba8, ImageError> {
        let (width, height) = (size.width.0, size.height.0);
        let mut pixmap =
            tiny_skia::Pixmap::new(width, height).ok_or(ImageError::TooLarge { size })?;
        let intrinsic = self.0.size();
        let transform = tiny_skia::Transform::from_scale(
            width as f32 / intrinsic.width(),
            height as f32 / intrinsic.height(),
        );
        resvg::render(&self.0, transform, &mut pixmap.as_mut());
        // tiny-skia draws premultiplied.
        Ok(PremultipliedRgba8::new(size, pixmap.take())?.unpremultiplied())
    }
}

/// The size a full view of an SVG is drawn at: its own size, enlarged so the long edge is at least
/// [`SVG_LONG_EDGE_MIN`] (a 16 px icon still looks sharp when zoomed) and reduced to at most
/// [`SVG_LONG_EDGE_MAX`] and [`MAX_DECODE_AREA`] pixels.
pub(crate) fn view_size(intrinsic: PixelSize) -> PixelSize {
    let long = intrinsic.width.0.max(intrinsic.height.0).max(1);
    let wanted = long.clamp(SVG_LONG_EDGE_MIN, SVG_LONG_EDGE_MAX);
    let scale = |side: u32| {
        let value = (u64::from(side) * u64::from(wanted) + u64::from(long) / 2) / u64::from(long);
        PixelLen(u32::try_from(value.max(1)).unwrap_or(u32::MAX))
    };
    fit_area(
        PixelSize {
            width: scale(intrinsic.width.0),
            height: scale(intrinsic.height.0),
        },
        MAX_DECODE_AREA,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> PixelSize {
        PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        }
    }

    #[test]
    fn a_full_view_is_drawn_between_the_minimum_and_maximum_long_edge() {
        // name, intrinsic, view
        type Case = (&'static str, (u32, u32), (u32, u32));
        const CASES: &[Case] = &[
            ("an icon is enlarged", (16, 16), (1024, 1024)),
            ("a wide icon keeps proportions", (64, 32), (1024, 512)),
            ("inside the range", (2000, 1000), (2000, 1000)),
            ("a poster is reduced", (8000, 4000), (4096, 2048)),
            (
                "a thin strip stays one pixel thick at least",
                (4, 4000),
                (4, 4000),
            ),
        ];
        for (name, (w, h), (vw, vh)) in CASES {
            assert_eq!(view_size(size(*w, *h)), size(*vw, *vh), "{name}");
        }
    }
}
