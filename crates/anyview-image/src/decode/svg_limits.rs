//! What a parsed SVG would cost to draw, worked out from its tree before any of it is drawn.
//!
//! A drawing can be a few hundred bytes and still ask for hours: a turbulence of a thousand
//! octaves, a morphology of radius two thousand, a blur over a region ten thousand times the page.
//! The cost of each filter primitive is its region in output pixels times a weight for the work it
//! does per pixel; the drawing is refused when the sum, over every element that is drawn, is more
//! than a few seconds of work.

use super::ceiling::Ceiling;
use super::stills::{STILL_COPIES, header};
use crate::error::ImageError;
use image::ImageFormat;
use resvg::usvg::{self, filter::Kind};

/// The most work a drawing may ask for, in the weights below: a few seconds.
const WORK_LIMIT: f64 = 3.0e9;

/// The most octaves a turbulence is drawn with. Each octave halves the one before, so past ten the
/// picture no longer changes; browsers stop there too.
const OCTAVE_LIMIT: u32 = 10;

fn heavy(reason: &str) -> ImageError {
    ImageError::Decode {
        reason: reason.to_owned(),
    }
}

/// `Ok` when `tree`, drawn with its units turned into pixels by `scale`, is within the limits.
pub(super) fn check(tree: &usvg::Tree, scale: (f32, f32)) -> Result<(), ImageError> {
    let mut work = 0.0;
    group(tree.root(), scale, &mut work)
}

fn group(parent: &usvg::Group, scale: (f32, f32), work: &mut f64) -> Result<(), ImageError> {
    filters(parent, scale, work)?;
    if *work > WORK_LIMIT {
        return Err(heavy("the drawing asks for too much work"));
    }
    for child in parent.children() {
        match child {
            usvg::Node::Group(inner) => group(inner, scale, work)?,
            usvg::Node::Image(image) => picture(image.kind(), scale, work)?,
            usvg::Node::Path(_) | usvg::Node::Text(_) => {}
        }
    }
    Ok(())
}

/// A picture inside the drawing: a raster is checked by its header, since it is decoded at the
/// size it declares, and a drawing inside the drawing by the same rules.
fn picture(kind: &usvg::ImageKind, scale: (f32, f32), work: &mut f64) -> Result<(), ImageError> {
    let (data, format) = match kind {
        usvg::ImageKind::SVG(inner) => return group(inner.root(), scale, work),
        usvg::ImageKind::PNG(data) => (data, ImageFormat::Png),
        usvg::ImageKind::JPEG(data) => (data, ImageFormat::Jpeg),
        usvg::ImageKind::GIF(data) => (data, ImageFormat::Gif),
        usvg::ImageKind::WEBP(data) => (data, ImageFormat::WebP),
    };
    // A header that cannot be read is the drawing's own problem: it draws nothing.
    if let Ok(found) = header(data, format) {
        let (size, per_pixel) = found.cost(STILL_COPIES);
        Ceiling::PEEK.admit(size, per_pixel)?;
    }
    Ok(())
}

fn filters(parent: &usvg::Group, scale: (f32, f32), work: &mut f64) -> Result<(), ImageError> {
    if parent.filters().is_empty() {
        return Ok(());
    }
    let (x, y) = parent.abs_transform().get_scale();
    let (x, y) = (f64::from(x * scale.0).abs(), f64::from(y * scale.1).abs());
    for filter in parent.filters() {
        let region = f64::from(filter.rect().width()) * x * f64::from(filter.rect().height()) * y;
        for primitive in filter.primitives() {
            *work += region * weight(primitive.kind(), (x, y))?;
        }
    }
    Ok(())
}

/// The work per pixel of the region that a primitive does, drawn at `scale` pixels to the unit.
fn weight(kind: &Kind, (x, y): (f64, f64)) -> Result<f64, ImageError> {
    Ok(match kind {
        Kind::Turbulence(turbulence) => {
            if turbulence.num_octaves() > OCTAVE_LIMIT {
                return Err(heavy("a turbulence has too many octaves"));
            }
            32.0 * f64::from(turbulence.num_octaves().max(1))
        }
        Kind::Morphology(morphology) => {
            let reach_x = f64::from(morphology.radius_x().get()) * x;
            let reach_y = f64::from(morphology.radius_y().get()) * y;
            2.0 * (1.0 + reach_x) * (1.0 + reach_y)
        }
        Kind::ConvolveMatrix(convolve) => {
            let cells =
                f64::from(convolve.matrix().columns()) * f64::from(convolve.matrix().rows());
            4.0 * cells.max(1.0)
        }
        Kind::GaussianBlur(_) | Kind::DropShadow(_) => 40.0,
        Kind::DiffuseLighting(_) | Kind::SpecularLighting(_) => 60.0,
        Kind::DisplacementMap(_) => 16.0,
        Kind::Blend(_)
        | Kind::ColorMatrix(_)
        | Kind::ComponentTransfer(_)
        | Kind::Composite(_)
        | Kind::Flood(_)
        | Kind::Image(_)
        | Kind::Merge(_)
        | Kind::Offset(_)
        | Kind::Tile(_) => 8.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(body: &str) -> usvg::Tree {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"1000\">{body}</svg>"
        );
        usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap()
    }

    fn filtered(primitive: &str, region: &str) -> String {
        format!(
            "<filter id=\"f\" {region}>{primitive}</filter><rect width=\"1000\" height=\"1000\" filter=\"url(#f)\"/>"
        )
    }

    #[test]
    fn a_drawing_is_refused_when_its_filters_ask_for_too_much() {
        let wide = "x=\"-1000%\" y=\"-1000%\" width=\"2000%\" height=\"2000%\"";
        // name, body, scale, drawn
        let cases: Vec<(&str, String, f32, bool)> = vec![
            (
                "no filter",
                "<rect width=\"9\" height=\"9\"/>".to_owned(),
                1.0,
                true,
            ),
            (
                "a blur",
                filtered("<feGaussianBlur stdDeviation=\"5\"/>", ""),
                1.0,
                true,
            ),
            (
                "ten octaves",
                filtered(
                    "<feTurbulence baseFrequency=\"0.05\" numOctaves=\"10\"/>",
                    "",
                ),
                1.0,
                true,
            ),
            (
                "eleven octaves",
                filtered(
                    "<feTurbulence baseFrequency=\"0.05\" numOctaves=\"11\"/>",
                    "",
                ),
                1.0,
                false,
            ),
            (
                "a thousand octaves",
                filtered(
                    "<feTurbulence baseFrequency=\"0.05\" numOctaves=\"1000\"/>",
                    "",
                ),
                0.25,
                false,
            ),
            (
                "a small morphology",
                filtered("<feMorphology radius=\"3\"/>", ""),
                1.0,
                true,
            ),
            (
                "a huge morphology",
                filtered("<feMorphology radius=\"2000\"/>", ""),
                1.0,
                false,
            ),
            (
                "a blur over a region of ten thousand pages",
                filtered("<feGaussianBlur stdDeviation=\"5000\"/>", wide),
                0.25,
                false,
            ),
            (
                "the same blur drawn tiny",
                filtered("<feGaussianBlur stdDeviation=\"5\"/>", wide),
                0.001,
                true,
            ),
            (
                "a big convolve matrix",
                filtered("<feConvolveMatrix order=\"60\" kernelMatrix=\"1\"/>", ""),
                1.0,
                true,
            ),
        ];
        for (name, body, scale, drawn) in cases {
            let got = check(&tree(&body), (scale, scale));
            assert_eq!(got.is_ok(), drawn, "{name}: {got:?}");
        }
    }

    #[test]
    fn a_filter_used_many_times_adds_up() {
        let uses = "<use href=\"#g\"/>".repeat(200);
        let body = format!(
            "<filter id=\"f\"><feGaussianBlur stdDeviation=\"5\"/></filter><g id=\"g\"><rect width=\"1000\" height=\"1000\" filter=\"url(#f)\"/></g>{uses}"
        );
        assert!(check(&tree(&body), (4.0, 4.0)).is_err());
        assert!(check(&tree(&body), (0.01, 0.01)).is_ok());
    }
}
