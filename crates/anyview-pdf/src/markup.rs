//! Writing on a PDF: the fields a person filled and the marks they made, saved as standard form
//! values and annotations that other viewers show, and the words of a page that a mark can be
//! laid over.

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::forms::filled;
use crate::geometry::displayed;
use anyview_core::{
    Mark, MarkColor, MarkShape, MarkupSave, NOTE_SIZE, PageIndex, PagePoint, PageRect,
};
use pdfrum::{Color, Point, Rect, Rotation};
use pdfrum_edit::{
    AnnotBorder, AnnotWrite, CircleSpec, EditDoc, FreeTextSpec, InkSpec, LineEndingStyle, LineSpec,
    MarkupKind, MarkupSpec, Quad, SaveMode, SaveOptions, SquareSpec, TextSpec, add_annotation,
};

/// How thick a drawn outline or line is, in points.
const STROKE: f32 = 2.0;

/// `doc` with what the person wrote on it, as the bytes of a new file: the form values first and
/// the marks after, each appended to the original so that it stays at the front unchanged. The
/// open document is untouched, and nothing is returned if any part fails.
pub fn write_markup(doc: &PdfDocument, save: &MarkupSave) -> Result<Vec<u8>, PdfError> {
    let mut bytes = doc.bytes().to_vec();
    if !save.fills.is_empty() {
        bytes = filled(doc, &save.fills)?;
    }
    if !save.marks.is_empty() {
        let current = PdfDocument::from_bytes(bytes)?;
        bytes = marked(&current, &save.marks)?;
    }
    Ok(bytes)
}

/// The boxes of the words of `page`, in reading order, on the page as displayed. A word with no
/// visible box is left out.
pub fn page_words(doc: &PdfDocument, page: PageIndex) -> Result<Vec<PageRect>, PdfError> {
    let sheet = doc.page(page)?;
    let (crop, rotation) = (sheet.crop_box(), sheet.rotation());
    Ok(sheet
        .words()
        .into_iter()
        .filter(|word| word.rect.width() > 0.0 && word.rect.height() > 0.0)
        .map(|word| displayed(word.rect, crop, rotation))
        .collect())
}

/// `marks` added to `doc` as annotations, appended to the file.
fn marked(doc: &PdfDocument, marks: &[Mark]) -> Result<Vec<u8>, PdfError> {
    let mut edit = EditDoc::new(doc.inner().parser());
    for mark in marks {
        let sheet = doc.page(mark.page)?;
        let place = Place {
            crop: sheet.crop_box(),
            rotation: sheet.rotation(),
        };
        if let Some(write) = annotation_of(mark, &place) {
            add_annotation(&mut edit, mark.page.0, write)?;
        }
    }
    let mut out = Vec::new();
    pdfrum_edit::save(
        &edit,
        &SaveOptions::builder().mode(SaveMode::Incremental).build(),
        &mut out,
    )?;
    Ok(out)
}

/// A page as the file lays it out: the crop box in user space, and how far the page turns when
/// shown. Turns a point of the page as displayed back into user space.
#[derive(Debug, Clone, Copy)]
struct Place {
    crop: Rect,
    rotation: Rotation,
}

impl Place {
    /// `at`, a point of the page as displayed, in the file's user space (y up).
    fn point(self, at: PagePoint) -> Point {
        let (a, b) = (f64::from(at.x.0) / 1000.0, f64::from(at.y.0) / 1000.0);
        // The inverse of `displayed`: the turn undone, then the unturned page (y down) laid on
        // the crop box.
        let (u, v) = match self.rotation {
            Rotation::None => (a, b),
            Rotation::Quarter => (b, 1.0 - a),
            Rotation::Half => (1.0 - a, 1.0 - b),
            Rotation::ThreeQuarter => (1.0 - b, a),
        };
        Point::new(
            self.crop.x0 + u * self.crop.width(),
            self.crop.y1 - v * self.crop.height(),
        )
    }

    /// `rect`, a rectangle of the page as displayed, in user space.
    fn rect(self, rect: PageRect) -> Rect {
        Rect::from_points(
            self.point(PagePoint {
                x: rect.left,
                y: rect.top,
            }),
            self.point(PagePoint {
                x: rect.right,
                y: rect.bottom,
            }),
        )
    }
}

fn color_of(color: MarkColor) -> Color {
    let (red, green, blue) = color.rgb();
    Color::from_rgb8(red, green, blue)
}

/// The annotation that is `mark`; `None` for a text mark that covers no text.
fn annotation_of(mark: &Mark, place: &Place) -> Option<AnnotWrite> {
    let color = color_of(mark.color);
    let border = AnnotBorder::solid(STROKE);
    match &mark.shape {
        MarkShape::Highlight(rects) => text_markup(MarkupKind::Highlight, rects, color, place),
        MarkShape::Underline(rects) => text_markup(MarkupKind::Underline, rects, color, place),
        MarkShape::Strikethrough(rects) => text_markup(MarkupKind::StrikeOut, rects, color, place),
        MarkShape::Note { at, text } => {
            let icon = PageRect::spanning(
                *at,
                at.moved(
                    i32::try_from(NOTE_SIZE.0).unwrap_or(0),
                    i32::try_from(NOTE_SIZE.1).unwrap_or(0),
                ),
            );
            Some(
                TextSpec::new(place.rect(icon), color)
                    .contents(text.clone())
                    .into(),
            )
        }
        MarkShape::Rectangle(rect) => Some(
            SquareSpec::new(place.rect(*rect), color)
                .border(border)
                .into(),
        ),
        MarkShape::Oval(rect) => Some(
            CircleSpec::new(place.rect(*rect), color)
                .border(border)
                .into(),
        ),
        MarkShape::Line { from, to } => Some(line(place, color, *from, *to, false)),
        MarkShape::Arrow { from, to } => Some(line(place, color, *from, *to, true)),
        MarkShape::Ink(strokes) => ink(place, color, strokes, border),
        MarkShape::TextBox { rect, text } => {
            Some(FreeTextSpec::new(place.rect(*rect), color, text.clone()).into())
        }
    }
}

/// A highlight, underline or strike-out over `rects`.
fn text_markup(
    kind: MarkupKind,
    rects: &[PageRect],
    color: Color,
    place: &Place,
) -> Option<AnnotWrite> {
    let covered: Vec<Rect> = rects.iter().map(|rect| place.rect(*rect)).collect();
    let bounds = covered.iter().copied().reduce(|a, b| a.union(b))?;
    Some(
        MarkupSpec::new(kind, bounds, color)
            .quads(covered.into_iter().map(Quad::from))
            .into(),
    )
}

/// Freehand `strokes`; `None` when they hold no point.
fn ink(
    place: &Place,
    color: Color,
    strokes: &[Vec<PagePoint>],
    border: AnnotBorder,
) -> Option<AnnotWrite> {
    let placed: Vec<Vec<Point>> = strokes
        .iter()
        .map(|stroke| stroke.iter().map(|point| place.point(*point)).collect())
        .collect();
    let bounds = placed
        .iter()
        .flatten()
        .map(|point| Rect::from_points(*point, *point))
        .reduce(|a, b| a.union(b))?;
    Some(InkSpec::new(bounds, color, placed).border(border).into())
}

/// A line from `from` to `to`, with an arrow head at its end when `arrow`.
fn line(place: &Place, color: Color, from: PagePoint, to: PagePoint, arrow: bool) -> AnnotWrite {
    let (start, end) = (place.point(from), place.point(to));
    let spec = LineSpec::new(Rect::from_points(start, end), color, start, end)
        .border(AnnotBorder::solid(STROKE));
    if arrow {
        spec.endings(LineEndingStyle::None, LineEndingStyle::OpenArrow)
            .into()
    } else {
        spec.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::displayed;

    #[test]
    fn a_rectangle_of_the_page_as_displayed_goes_back_where_it_came_from() {
        // The crop box is 200 x 100 with its corner at (10, 20); the box lies in the top left
        // fifth of the page, and the page may be turned.
        let crop = Rect::new(10.0, 20.0, 210.0, 120.0);
        let probe = Rect::new(30.0, 90.0, 70.0, 110.0);
        for rotation in [
            Rotation::None,
            Rotation::Quarter,
            Rotation::Half,
            Rotation::ThreeQuarter,
        ] {
            let shown = displayed(probe, crop, rotation);
            let back = Place { crop, rotation }.rect(shown);
            for (got, want) in [
                (back.x0, probe.x0),
                (back.y0, probe.y0),
                (back.x1, probe.x1),
                (back.y1, probe.y1),
            ] {
                // A thousandth of a 200 point page is a fifth of a point.
                assert!(
                    (got - want).abs() <= 0.2,
                    "{rotation:?}: {back:?} vs {probe:?}"
                );
            }
        }
    }
}
