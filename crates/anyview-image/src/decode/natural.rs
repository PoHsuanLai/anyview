//! The size a picture will show at, read from the start of its file: what a window is sized to
//! before it opens. Nothing is decoded and at most [`HEAD`] bytes are read, so it costs a few
//! milliseconds whatever the file claims.

use super::codec::{Codec, codec_for};
use super::stills;
use anyview_core::{Input, PixelLen, PixelSize, QuarterTurn, Sniffed};

/// The most of a file read for its size. A JPEG's frame header follows its metadata segments (the
/// EXIF block and a thumbnail inside it can fill most of 64 KiB), so a larger window is read.
const HEAD: u64 = 256 * 1024;

/// The size the picture in `src` (a path, or any bytes a host injects) shows at, upright, in pixels, as the head of the file declares
/// it; `sniffed` is what the file was established to be. `None` when the format does not say in its
/// head (ICNS, JPEG XL, camera raw, a layered document), the head is damaged or cut before the
/// size, or the path is not a regular file. The size is the file's claim: nothing checks it fits
/// a decode. An SVG gives the size it declares, rounded up to whole pixels.
/// Blocking: run it on a worker.
pub fn natural_size(src: impl Into<Input>, sniffed: &Sniffed) -> Option<PixelSize> {
    let head = src.into().bytes().read_range(0..HEAD).ok()?;
    natural_size_of(&head, sniffed)
}

/// [`natural_size`] of the head of a file already read.
pub(crate) fn natural_size_of(head: &[u8], sniffed: &Sniffed) -> Option<PixelSize> {
    match codec_for(sniffed).ok()? {
        Codec::Image(format) | Codec::HighRange(format) => {
            let size = stills::header(head, format).ok()?.size;
            let turn = crate::exif::ExifFacts::read(head).orientation.turn;
            Some(match turn {
                QuarterTurn::None | QuarterTurn::Half => size,
                QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => PixelSize {
                    width: size.height,
                    height: size.width,
                },
            })
        }
        Codec::Svg => declared_by_svg(head),
        Codec::Psd | Codec::Icns | Codec::Jxl | Codec::RawPreview => None,
    }
}

/// What the root `<svg>` element's `width`, `height` and `viewBox` say, with the rules a drawing
/// follows: a missing side comes from the other and the view box's ratio, none at all is the view
/// box's size. Lengths in `em`, `ex` or per cent depend on a context a header lacks, so they give
/// nothing.
fn declared_by_svg(head: &[u8]) -> Option<PixelSize> {
    let text = String::from_utf8_lossy(head);
    let tag = root_tag(&text)?;
    let attribute = |name: &str| {
        attributes(tag)
            .find(|(key, _)| *key == name)
            .map(|(_, v)| v)
    };
    let width = attribute("width").map(length);
    let height = attribute("height").map(length);
    let view = attribute("viewBox").and_then(view_box);
    let (width, height) = match (width, height, view) {
        (Some(Some(w)), Some(Some(h)), _) => (w, h),
        (Some(Some(w)), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(Some(h)), Some((vw, vh))) => (h * vw / vh, h),
        (None, None, Some((vw, vh))) => (vw, vh),
        _ => return None,
    };
    let whole = |length: f64| {
        (length.is_finite() && length > 0.0).then(|| PixelLen((length.ceil() as u32).max(1)))
    };
    Some(PixelSize {
        width: whole(width)?,
        height: whole(height)?,
    })
}

/// The inside of the first element's start tag, after skipping the prolog's declarations, comments
/// and doctype; `None` when that element is not `svg` or the head ends inside it.
fn root_tag(text: &str) -> Option<&str> {
    let mut rest = text;
    loop {
        let open = rest.find('<')?;
        rest = &rest[open..];
        let skip_to = |end: &str| rest.find(end).map(|at| &rest[at + end.len()..]);
        if rest.starts_with("<!--") {
            rest = skip_to("-->")?;
        } else if rest.starts_with("<?") {
            rest = skip_to("?>")?;
        } else if rest.starts_with("<!") {
            // A doctype may hold an internal subset in brackets, closed by `]>`.
            rest = if rest.contains('[') && rest.find('[') < rest.find('>') {
                skip_to("]>")?
            } else {
                skip_to(">")?
            };
        } else {
            break;
        }
    }
    let body = rest.strip_prefix("<svg")?;
    if !body.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
        return None;
    }
    // The tag ends at the first `>` outside a quoted value.
    let mut quote = None;
    for (at, c) in body.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return Some(&body[..at]),
            _ => {}
        }
    }
    None
}

/// The `name="value"` pairs of a start tag, in order; a malformed tail ends them.
fn attributes(tag: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut rest = tag;
    std::iter::from_fn(move || {
        let eq = rest.find('=')?;
        let name = rest[..eq].trim().rsplit(char::is_whitespace).next()?;
        let after = rest[eq + 1..].trim_start();
        let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let inner = &after[1..];
        let end = inner.find(quote)?;
        rest = &inner[end + 1..];
        Some((name, &inner[..end]))
    })
}

/// A length in CSS pixels at 96 to the inch; `None` for a unit that needs a context (`em` and `ex`
/// read as a broken exponent, which is no length either).
fn length(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number.parse().ok()?;
    let per_unit = match unit.trim() {
        "" | "px" => 1.0,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        "mm" => 96.0 / 25.4,
        "cm" => 96.0 / 2.54,
        "in" => 96.0,
        _ => return None,
    };
    (value > 0.0).then_some(value * per_unit)
}

/// The width and height of a `viewBox` value.
fn view_box(text: &str) -> Option<(f64, f64)> {
    let mut numbers = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse::<f64>);
    let (_, _) = (numbers.next()?.ok()?, numbers.next()?.ok()?);
    let (width, height) = (numbers.next()?.ok()?, numbers.next()?.ok()?);
    (numbers.next().is_none() && width > 0.0 && height > 0.0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(tag: &str) -> Option<(u32, u32)> {
        let doc = format!(
            "<?xml version=\"1.0\"?>\n<!-- <svg width=\"1\"> -->\n<svg {tag}><rect/></svg>"
        );
        declared_by_svg(doc.as_bytes()).map(|s| (s.width.0, s.height.0))
    }

    #[test]
    fn an_svg_declares_its_size_as_a_drawing_reads_it() {
        assert_eq!(svg(r#"width="64" height="32""#), Some((64, 32)));
        assert_eq!(svg(r#"width='2in' height="1.5in""#), Some((192, 144)));
        assert_eq!(svg(r#"viewBox="0 0 24 12""#), Some((24, 12)));
        assert_eq!(svg(r#"width="48" viewBox="0 0 24 12""#), Some((48, 24)));
        assert_eq!(svg(r#"height="10.2" viewBox="0 0 20 10""#), Some((21, 11)));
        assert_eq!(svg(r#"width="100%" height="100%""#), None);
        assert_eq!(svg(r#"width="2em" height="3em" viewBox="0 0 8 8""#), None);
        assert_eq!(svg(r#"width="0" height="5""#), None);
        assert_eq!(svg(""), None);
    }

    #[test]
    fn a_root_that_is_not_svg_or_is_cut_off_has_no_size() {
        assert_eq!(declared_by_svg(b"<html width=\"4\" height=\"4\">"), None);
        assert_eq!(declared_by_svg(b"<svg width=\"4\" height=\"4\""), None);
        assert_eq!(declared_by_svg(b"<svgx width=\"4\" height=\"4\">"), None);
        assert_eq!(declared_by_svg(&[0xFF, 0xFE, 0x00]), None);
    }
}
