//! SVG through `resvg`: rendered to straight RGBA8 at a size the caller picks.

use super::svg_limits;
use super::{MAX_DECODE_AREA, SVG_LONG_EDGE_MAX, SVG_LONG_EDGE_MIN};
use crate::error::ImageError;
use crate::pixels::{PremultipliedRgba8, Rgba8};
use crate::scale::fit_area;
use anyview_core::{MAX_XML_DEPTH, PixelLen, PixelSize, nests_deeper_than};
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
    /// are not read: an `href` that is not a `data:` URL resolves to nothing, so only what the SVG
    /// itself contains is shown. A document nested deeper than [`MAX_XML_DEPTH`] is refused
    /// before the parser, which recurses once per open element, sees it.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Svg, ImageError> {
        if std::str::from_utf8(bytes).is_ok_and(|text| nests_deeper_than(text, MAX_XML_DEPTH)) {
            return Err(decode_error("the drawing nests too deeply"));
        }
        let options = usvg::Options {
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
                resolve_string: Box::new(|_, _| None),
            },
            ..usvg::Options::default()
        };
        usvg::Tree::from_data(bytes, &options)
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
        svg_limits::check(
            &self.0,
            (
                width as f32 / intrinsic.width(),
                height as f32 / intrinsic.height(),
            ),
        )?;
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

    /// How many pictures the parsed drawing holds: an `<image>` whose `href` resolved.
    fn pictures(svg: &Svg) -> usize {
        fn count(group: &usvg::Group) -> usize {
            group
                .children()
                .iter()
                .map(|node| match node {
                    usvg::Node::Image(_) => 1,
                    usvg::Node::Group(inner) => count(inner),
                    usvg::Node::Path(_) | usvg::Node::Text(_) => 0,
                })
                .sum()
        }
        count(svg.0.root())
    }

    fn drawing_of(href: &str) -> Svg {
        let text = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
             width=\"8\" height=\"8\"><image width=\"8\" height=\"8\" xlink:href=\"{href}\"/></svg>"
        );
        Svg::parse(text.as_bytes()).unwrap()
    }

    #[test]
    fn an_href_is_resolved_only_when_it_is_a_data_url() {
        let dir = tempfile::tempdir().unwrap();
        let red = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        let path = dir.path().join("red.png");
        red.save(&path).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        red.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let inline = format!(
            "data:image/png;base64,{}",
            ds_core::base64::encode(png.get_ref())
        );
        // name, href, pictures the drawing holds
        let cases: Vec<(&str, String, usize)> = vec![
            ("a data url is read", inline, 1),
            ("an absolute path is not", path.display().to_string(), 0),
            (
                "a path relative to the working directory is not",
                "Cargo.toml".to_owned(),
                0,
            ),
            ("a file URL is not", format!("file://{}", path.display()), 0),
            ("a special file is not", "/dev/zero".to_owned(), 0),
        ];
        for (name, href, want) in cases {
            assert_eq!(pictures(&drawing_of(&href)), want, "{name}");
        }
    }

    #[test]
    fn a_nested_drawing_is_refused_before_the_parser_sees_it() {
        for depth in [300, 6000, 100_000] {
            let text = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\">{}",
                "<g>".repeat(depth)
            );
            assert!(Svg::parse(text.as_bytes()).is_err(), "depth {depth}");
        }
        let fine = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\">{}{}</svg>",
            "<g>".repeat(120),
            "</g>".repeat(120)
        );
        assert!(Svg::parse(fine.as_bytes()).is_ok());
    }

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
