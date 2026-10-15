//! Writing on a PDF: the fields a person filled and the marks they made, saved as standard form
//! values and annotations that other viewers show, the annotations a file already holds read as
//! marks that can be moved and deleted, and the words of a page that a mark can be laid over.

use crate::document::PdfDocument;
use crate::error::PdfError;
use crate::forms::filled;
use crate::geometry::displayed;
use anyview_core::{
    Mark, MarkColor, MarkRef, MarkShape, MarkupSave, NOTE_SIZE, PageIndex, PagePoint, PageRect,
};
use pdfrum::{Annotation, Color, ObjRef, Point, Rect, Rotation, Subtype};
use pdfrum_edit::{
    AnnotBorder, AnnotWrite, CircleSpec, EditDoc, FreeTextSpec, InkSpec, LineEndingStyle, LineSpec,
    MarkupKind, MarkupSpec, Quad, SaveMode, SaveOptions, SquareSpec, TextSpec, add_annotation,
    delete_annotation, modify_annotation, move_annotation,
};
use pdfrum_object::{Array, Dict, Name, PdfString, Resolve, encode_text};

/// How thick a drawn outline or line is, in points.
const STROKE: f32 = 2.0;

/// `/C`, the colour of an annotation.
const COLOR: Name = Name::from_static(b"C");
/// `/IC`, the interior colour.
const INTERIOR: Name = Name::from_static(b"IC");
/// `/Contents`, the words of a note or text box.
const CONTENTS: Name = Name::from_static(b"Contents");
/// `/L`, the two ends of a line, and `/LE`, how each ends.
const LINE: Name = Name::from_static(b"L");
const LINE_ENDINGS: Name = Name::from_static(b"LE");
/// `/InkList`, the strokes of an ink annotation.
const INK_LIST: Name = Name::from_static(b"InkList");

/// `doc` with what the person wrote on it, as the bytes of a new file: the form values first and
/// the marks after, each appended to the original so that it stays at the front unchanged. The
/// open document is untouched, and nothing is returned if any part fails.
pub fn write_markup(doc: &PdfDocument, save: &MarkupSave) -> Result<Vec<u8>, PdfError> {
    let mut bytes = doc.bytes().to_vec();
    if !save.fills.is_empty() {
        bytes = filled(doc, &save.fills)?;
    }
    if !save.marks.is_empty() || !save.removed.is_empty() {
        let current = PdfDocument::from_bytes(bytes)?;
        bytes = marked(&current, &save.marks, &save.removed)?;
    }
    Ok(bytes)
}

/// The annotations `doc` already holds that a mark can stand for, in the file's order, each with
/// the reference the file lists it by: squares, circles, lines and arrows, ink, text markup,
/// notes and free text. Form widgets, links, pop-ups, hidden annotations, ones written inline in
/// the page (which cannot be edited by reference) and kinds of annotation with no mark are left
/// alone, and so are lines with something other than an arrow head at their end.
pub fn existing_marks(doc: &PdfDocument) -> Vec<Mark> {
    let parser = doc.inner().parser();
    let mut marks = Vec::new();
    for index in 0..doc.page_count().get() {
        let page = PageIndex(index);
        let Ok(sheet) = doc.page(page) else {
            continue;
        };
        let place = Place {
            crop: sheet.crop_box(),
            rotation: sheet.rotation(),
        };
        for annotation in sheet.annotations() {
            let Some(held) = annotation.obj_ref().filter(|_| !annotation.is_hidden()) else {
                continue;
            };
            let Some(shape) = shape_of(&annotation, &place, parser) else {
                continue;
            };
            let color = color_in(annotation.dict(), parser)
                .map_or_else(MarkColor::default, |c| MarkColor::nearest(c.0, c.1, c.2));
            marks.push(Mark {
                page,
                color,
                shape,
                existing: Some(MarkRef {
                    number: held.num,
                    generation: held.generation,
                }),
            });
        }
    }
    marks
}

/// What `annotation` is as a mark; `None` for a kind that no mark stands for.
fn shape_of(annotation: &Annotation<'_>, place: &Place, r: &impl Resolve) -> Option<MarkShape> {
    let rect = place.shown_rect(annotation.rect());
    let dict = annotation.dict();
    match annotation.subtype() {
        Subtype::Square => Some(MarkShape::Rectangle(rect)),
        Subtype::Circle => Some(MarkShape::Oval(rect)),
        Subtype::Highlight => quad_lines(annotation, place).map(MarkShape::Highlight),
        Subtype::Underline => quad_lines(annotation, place).map(MarkShape::Underline),
        Subtype::StrikeOut => quad_lines(annotation, place).map(MarkShape::Strikethrough),
        Subtype::Text => Some(MarkShape::Note {
            at: PagePoint {
                x: rect.left,
                y: rect.top,
            },
            text: annotation.contents().unwrap_or_default(),
        }),
        Subtype::FreeText => Some(MarkShape::TextBox {
            rect,
            text: annotation.contents().unwrap_or_default(),
        }),
        Subtype::Line => {
            let ends = numbers(&dict.array(&LINE, r)?);
            let [x0, y0, x1, y1] = ends[..] else {
                return None;
            };
            let from = place.shown_point(Point::new(x0, y0));
            let to = place.shown_point(Point::new(x1, y1));
            let endings = dict.array(&LINE_ENDINGS, r);
            let ending = |index: usize| {
                endings
                    .as_ref()
                    .and_then(|array| array.name_at(index))
                    .map_or(&b"None"[..], Name::as_bytes)
            };
            match (ending(0), ending(1)) {
                (b"None", b"None") => Some(MarkShape::Line { from, to }),
                (b"None", b"OpenArrow" | b"ClosedArrow") => Some(MarkShape::Arrow { from, to }),
                _ => None,
            }
        }
        Subtype::Ink => {
            let list = dict.array(&INK_LIST, r)?;
            let strokes: Vec<Vec<PagePoint>> = (0..list.len())
                .filter_map(|at| list.array_at(at, r))
                .map(|stroke| {
                    numbers(&stroke)
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&[x, y]| place.shown_point(Point::new(x, y)))
                        .collect()
                })
                .filter(|stroke: &Vec<PagePoint>| !stroke.is_empty())
                .collect();
            (!strokes.is_empty()).then_some(MarkShape::Ink(strokes))
        }
        Subtype::Unknown
        | Subtype::Link
        | Subtype::Polygon
        | Subtype::PolyLine
        | Subtype::Squiggly
        | Subtype::Stamp
        | Subtype::Caret
        | Subtype::Popup
        | Subtype::FileAttachment
        | Subtype::Sound
        | Subtype::Movie
        | Subtype::Widget
        | Subtype::Screen
        | Subtype::PrinterMark
        | Subtype::TrapNet
        | Subtype::Watermark
        | Subtype::ThreeD
        | Subtype::RichMedia
        | Subtype::XfaWidget
        | Subtype::Redact => None,
    }
}

/// The lines a text-markup annotation covers, one per quadrilateral; `None` when it has none.
fn quad_lines(annotation: &Annotation<'_>, place: &Place) -> Option<Vec<PageRect>> {
    let lines: Vec<PageRect> = annotation
        .quad_points()
        .map(|quad| place.shown_rect(quad))
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines)
}

/// The numbers of `array`, those that are numbers.
fn numbers(array: &Array) -> Vec<f64> {
    (0..array.len())
        .filter_map(|at| array.number_at(at))
        .map(f64::from)
        .collect()
}

/// The colour `/C` of an annotation, as red, green and blue; `None` for no colour.
fn color_in(dict: &Dict, r: &impl Resolve) -> Option<(u8, u8, u8)> {
    let channels = numbers(&dict.array(&COLOR, r)?);
    let byte = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    match channels[..] {
        [gray] => Some((byte(gray), byte(gray), byte(gray))),
        [red, green, blue] => Some((byte(red), byte(green), byte(blue))),
        [cyan, magenta, yellow, black] => {
            let ink = |value: f64| byte(1.0 - (value + black).min(1.0));
            Some((ink(cyan), ink(magenta), ink(yellow)))
        }
        _ => None,
    }
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

/// `removed` taken out of `doc`, the held marks among `marks` changed where they are, and the
/// others added to it as annotations, appended to the file.
///
/// A held mark is never rebuilt: the annotation keeps every key the file gave it. One that moved
/// is translated by the exact distance it moved (`move_annotation`), then one recoloured or
/// reworded has just that key set (`modify_annotation`). A held mark can only be moved,
/// recoloured or reworded, so no other change reaches here; one that did would be left as it is.
fn marked(doc: &PdfDocument, marks: &[Mark], removed: &[Mark]) -> Result<Vec<u8>, PdfError> {
    let mut edit = EditDoc::new(doc.inner().parser());
    // Removing first: pages may share one list of annotations, and a removal does not move what
    // a change or an addition then finds.
    for gone in removed {
        if let Some(held) = gone.existing {
            delete_annotation(
                &mut edit,
                gone.page.0,
                ObjRef::new(held.number, held.generation),
            )?;
        }
    }
    let opened = if marks.iter().any(|mark| mark.existing.is_some()) {
        existing_marks(doc)
    } else {
        Vec::new()
    };
    for mark in marks {
        let sheet = doc.page(mark.page)?;
        let place = Place {
            crop: sheet.crop_box(),
            rotation: sheet.rotation(),
        };
        match mark.existing {
            Some(held) => {
                let Some(base) = opened
                    .iter()
                    .find(|base| base.existing == mark.existing && base.page == mark.page)
                else {
                    continue;
                };
                let held = ObjRef::new(held.number, held.generation);
                change_held(&mut edit, &place, held, base, mark)?;
            }
            None => {
                if let Some(write) = annotation_of(mark, &place) {
                    add_annotation(&mut edit, mark.page.0, write)?;
                }
            }
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

/// What was done to the annotation `held`, which was `base` when the file was opened and is
/// `mark` now: moved, then recoloured and reworded.
fn change_held(
    edit: &mut EditDoc<'_>,
    place: &Place,
    held: ObjRef,
    base: &Mark,
    mark: &Mark,
) -> Result<(), PdfError> {
    let page = mark.page.0;
    if let Some((from, to)) = translation(place, base, mark) {
        move_annotation(edit, page, held, to - from)?;
    }
    let color = (base.color != mark.color).then(|| {
        let (red, green, blue) = mark.color.rgb();
        Array::of([red, green, blue].map(|channel| f32::from(channel) / 255.0))
    });
    let words = match (words_of(&base.shape), words_of(&mark.shape)) {
        (Some(before), Some(after)) if before != after => Some(after.to_owned()),
        _ => None,
    };
    if color.is_some() || words.is_some() {
        modify_annotation(edit, page, held, |dict: &mut Dict| {
            if let Some(color) = color {
                if dict.contains_key(&INTERIOR) {
                    dict.insert(INTERIOR, color.clone());
                }
                dict.insert(COLOR, color);
            }
            if let Some(words) = words {
                dict.insert(CONTENTS, PdfString::literal(encode_text(&words)));
            }
        })?;
    }
    Ok(())
}

/// Where the top left of `base` and of `mark` are in user space, when `mark` is `base` moved and nothing
/// else changed (the words of a note or text box aside); `None` when it did not move, or changed
/// in any other way. Their difference is the distance it moved. Worked out from the same corner of each, so a turned page or a crop box that does
/// not start at the origin is accounted for as in [`Place::point`].
fn translation(place: &Place, base: &Mark, mark: &Mark) -> Option<(Point, Point)> {
    let (from, to) = (base.bounds(), mark.bounds());
    let by = (
        i32::try_from(to.left.0).ok()? - i32::try_from(from.left.0).ok()?,
        i32::try_from(to.top.0).ok()? - i32::try_from(from.top.0).ok()?,
    );
    if by == (0, 0) || without_words(&base.moved(by.0, by.1).shape) != without_words(&mark.shape) {
        return None;
    }
    let corner = |rect: PageRect| {
        place.point(PagePoint {
            x: rect.left,
            y: rect.top,
        })
    };
    Some((corner(from), corner(to)))
}

/// The words of a note or text box.
fn words_of(shape: &MarkShape) -> Option<&str> {
    match shape {
        MarkShape::Note { text, .. } | MarkShape::TextBox { text, .. } => Some(text),
        MarkShape::Highlight(_)
        | MarkShape::Underline(_)
        | MarkShape::Strikethrough(_)
        | MarkShape::Rectangle(_)
        | MarkShape::Oval(_)
        | MarkShape::Line { .. }
        | MarkShape::Arrow { .. }
        | MarkShape::Ink(_) => None,
    }
}

/// `shape` with the words of a note or text box taken out, to compare where things are.
fn without_words(shape: &MarkShape) -> MarkShape {
    match shape.clone() {
        MarkShape::Note { at, .. } => MarkShape::Note {
            at,
            text: String::new(),
        },
        MarkShape::TextBox { rect, .. } => MarkShape::TextBox {
            rect,
            text: String::new(),
        },
        other @ (MarkShape::Highlight(_)
        | MarkShape::Underline(_)
        | MarkShape::Strikethrough(_)
        | MarkShape::Rectangle(_)
        | MarkShape::Oval(_)
        | MarkShape::Line { .. }
        | MarkShape::Arrow { .. }
        | MarkShape::Ink(_)) => other,
    }
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

    /// `point`, in user space, as a point of the page as displayed.
    fn shown_point(&self, point: Point) -> PagePoint {
        let shown = displayed(Rect::from_points(point, point), self.crop, self.rotation);
        PagePoint {
            x: shown.left,
            y: shown.top,
        }
    }

    /// `rect`, in user space, as a rectangle of the page as displayed.
    fn shown_rect(&self, rect: Rect) -> PageRect {
        displayed(rect, self.crop, self.rotation)
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
