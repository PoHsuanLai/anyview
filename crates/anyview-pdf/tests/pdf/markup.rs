//! What a person writes on a PDF, saved and read back: form values, and marks as standard
//! annotations.

use crate::support::{
    drawn, fixture, fixture_bytes, form_bytes, inherited_choice_bytes, pixel, run, tiles,
};

use anyview_core::{
    Dpi, FieldFill, Mark, MarkColor, MarkShape, MarkupSave, PageIndex, PagePoint, PageRect,
    Permille, WidgetAccess, WidgetKind,
};
use anyview_pdf::{
    End, PdfDocument, PdfError, PdfJob, Priority, TILE_SIDE, Ticket, TileBatch, TileCoord,
    ZoomBucket, existing_marks, form_widgets, page_words, write_markup,
};
use pdfrum::Subtype;

fn fill(name: &str, value: &str) -> FieldFill {
    FieldFill {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
    PageRect::new(
        Permille(left),
        Permille(top),
        Permille(right),
        Permille(bottom),
    )
}

fn mark(shape: MarkShape) -> Mark {
    Mark {
        page: PageIndex(0),
        color: MarkColor::Yellow,
        shape,
        existing: None,
    }
}

fn save(fills: Vec<FieldFill>, marks: Vec<Mark>) -> MarkupSave {
    MarkupSave {
        fills,
        marks,
        removed: Vec::new(),
    }
}

#[test]
fn the_widgets_are_read_with_their_kinds_in_the_documents_order() {
    let doc = PdfDocument::from_bytes(form_bytes()).unwrap();
    let widgets = form_widgets(&doc);
    let got: Vec<(&str, WidgetKind, WidgetAccess, &str)> = widgets
        .iter()
        .map(|w| (w.name.as_str(), w.kind, w.access, w.value.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("name", WidgetKind::Text, WidgetAccess::Editable, ""),
            ("agree", WidgetKind::Check, WidgetAccess::Editable, "Off"),
            ("colour", WidgetKind::Choice, WidgetAccess::Editable, "Red"),
        ]
    );
    assert_eq!(widgets[1].on, "Yes");
    assert_eq!(widgets[2].options, ["Red", "Green", "Blue"]);
    assert_eq!(widgets[2].exports, widgets[2].options);
    assert!(widgets.iter().all(|w| w.page == PageIndex(0)));
    // The text field is the 200 x 20 point box at the left, a quarter of the way down the page.
    let name = widgets[0].rect;
    assert_eq!((name.left().0, name.right().0), (118, 444));
    assert!(name.top().0 > 200 && name.top().0 < 300, "{name:?}");
}

#[test]
fn a_drop_down_that_inherits_its_options_writes_their_export_values() {
    let doc = PdfDocument::from_bytes(inherited_choice_bytes()).unwrap();
    let widgets = form_widgets(&doc);
    assert_eq!(widgets.len(), 1, "{widgets:?}");
    assert_eq!(widgets[0].kind, WidgetKind::Choice);
    assert_eq!(
        widgets[0].options,
        ["Small", "Large"],
        "the labels are shown"
    );
    assert_eq!(
        widgets[0].exports,
        ["s", "l"],
        "the export values are written"
    );
}

#[test]
fn a_page_without_a_form_has_no_widgets() {
    assert!(form_widgets(&fixture()).is_empty());
}

#[test]
fn filled_fields_are_appended_to_the_file_and_read_back() {
    let original = form_bytes();
    let doc = PdfDocument::from_bytes(original.clone()).unwrap();
    let fills = vec![
        fill("name", "Ada Lovelace"),
        fill("agree", "Yes"),
        fill("colour", "Green"),
    ];
    let bytes = write_markup(&doc, &save(fills, Vec::new())).unwrap();
    assert!(
        bytes.starts_with(&original),
        "the original stays at the front"
    );
    let reopened = PdfDocument::from_bytes(bytes).unwrap();
    let values: Vec<(String, String)> = form_widgets(&reopened)
        .into_iter()
        .map(|w| (w.name, w.value))
        .collect();
    assert_eq!(
        values,
        [
            ("name".to_owned(), "Ada Lovelace".to_owned()),
            ("agree".to_owned(), "Yes".to_owned()),
            ("colour".to_owned(), "Green".to_owned()),
        ]
    );
    // The open document is as it was.
    assert_eq!(form_widgets(&doc)[0].value, "");
}

#[test]
fn a_value_for_a_field_that_is_not_there_writes_nothing() {
    let doc = PdfDocument::from_bytes(form_bytes()).unwrap();
    let result = write_markup(&doc, &save(vec![fill("nickname", "x")], Vec::new()));
    assert!(matches!(result, Err(PdfError::NoField { name }) if name == "nickname"));
}

#[test]
fn marks_become_standard_annotations_other_viewers_show() {
    let doc = fixture();
    let words = rect(118, 120, 400, 140);
    let marks = vec![
        mark(MarkShape::Highlight(vec![words])),
        mark(MarkShape::Underline(vec![words])),
        mark(MarkShape::Strikethrough(vec![words])),
        mark(MarkShape::Note {
            at: PagePoint::at(700, 100),
            text: "Check this".to_owned(),
        }),
        mark(MarkShape::Rectangle(rect(100, 500, 300, 600))),
        mark(MarkShape::Oval(rect(400, 500, 600, 600))),
        mark(MarkShape::Line {
            from: PagePoint::at(100, 700),
            to: PagePoint::at(300, 760),
        }),
        mark(MarkShape::Arrow {
            from: PagePoint::at(400, 700),
            to: PagePoint::at(600, 760),
        }),
        mark(MarkShape::Ink(vec![vec![
            PagePoint::at(700, 700),
            PagePoint::at(750, 720),
            PagePoint::at(800, 700),
        ]])),
        mark(MarkShape::TextBox {
            rect: rect(100, 800, 400, 850),
            text: "Hello".to_owned(),
        }),
    ];
    let bytes = write_markup(&doc, &save(Vec::new(), marks)).unwrap();
    let reopened = pdfrum::Document::from_bytes(bytes).unwrap();
    let page = reopened.page(0).unwrap();
    let subtypes: Vec<Subtype> = page
        .annotations()
        .map(|annotation| annotation.subtype())
        .filter(|subtype| *subtype != Subtype::Link)
        .collect();
    assert_eq!(
        subtypes,
        [
            Subtype::Highlight,
            Subtype::Underline,
            Subtype::StrikeOut,
            Subtype::Text,
            Subtype::Square,
            Subtype::Circle,
            Subtype::Line,
            Subtype::Line,
            Subtype::Ink,
            Subtype::FreeText,
        ]
    );
    let note = page
        .annotations()
        .find(|annotation| annotation.subtype() == Subtype::Text)
        .unwrap();
    assert_eq!(note.contents().as_deref(), Some("Check this"));
    // The links that were there are still there.
    assert_eq!(page.page_links().len(), 2);
}

#[test]
fn a_highlight_lands_over_the_words_it_was_drawn_on() {
    let doc = fixture();
    let words = page_words(&doc, PageIndex(0)).unwrap();
    let first = *words.first().expect("the first page has words");
    let bytes = write_markup(
        &doc,
        &save(Vec::new(), vec![mark(MarkShape::Highlight(vec![first]))]),
    )
    .unwrap();
    let reopened = pdfrum::Document::from_bytes(bytes).unwrap();
    let page = reopened.page(0).unwrap();
    let highlight = page
        .annotations()
        .find(|annotation| annotation.subtype() == Subtype::Highlight)
        .unwrap();
    let (crop, rotation) = (page.crop_box(), page.rotation());
    let shown = anyview_pdf::displayed(highlight.rect(), crop, rotation);
    for (got, want) in [
        (shown.left().0, first.left().0),
        (shown.top().0, first.top().0),
        (shown.right().0, first.right().0),
        (shown.bottom().0, first.bottom().0),
    ] {
        assert!(got.abs_diff(want) <= 3, "{shown:?} vs {first:?}");
    }
}

#[test]
fn fields_and_marks_are_saved_together() {
    let doc = PdfDocument::from_bytes(form_bytes()).unwrap();
    let bytes = write_markup(
        &doc,
        &save(
            vec![fill("name", "Grace")],
            vec![mark(MarkShape::Rectangle(rect(100, 100, 200, 200)))],
        ),
    )
    .unwrap();
    let reopened = PdfDocument::from_bytes(bytes.clone()).unwrap();
    assert_eq!(form_widgets(&reopened)[0].value, "Grace");
    let annotated = pdfrum::Document::from_bytes(bytes).unwrap();
    assert!(
        annotated
            .page(0)
            .unwrap()
            .annotations()
            .any(|annotation| annotation.subtype() == Subtype::Square)
    );
}

/// The fixture with two squares on its first page, a circle on its second, and a text box on its
/// third, as a file another program might have left them: written by `write_markup`, then opened.
fn annotated() -> PdfDocument {
    let red = |page: u32, shape: MarkShape| Mark {
        page: PageIndex(page),
        color: MarkColor::Red,
        ..mark(shape)
    };
    let marks = vec![
        red(0, MarkShape::Rectangle(rect(100, 500, 300, 600))),
        red(0, MarkShape::Rectangle(rect(500, 500, 700, 600))),
        red(1, MarkShape::Oval(rect(100, 100, 300, 200))),
        red(
            2,
            MarkShape::TextBox {
                rect: rect(100, 100, 400, 150),
                text: "Hello".to_owned(),
            },
        ),
    ];
    let bytes = write_markup(&fixture(), &save(Vec::new(), marks)).unwrap();
    PdfDocument::from_bytes(bytes).unwrap()
}

/// How many annotations other than links `page` of `bytes` lists.
fn annotations_on(bytes: Vec<u8>, page: u32) -> usize {
    let reopened = pdfrum::Document::from_bytes(bytes).unwrap();
    reopened
        .page(page)
        .unwrap()
        .annotations()
        .filter(|annotation| annotation.subtype() != Subtype::Link)
        .count()
}

/// Whether the box of `raster` holds a pixel that is clearly red.
fn has_red(raster: &anyview_pdf::Raster, x: u32, y: u32, width: u32, height: u32) -> bool {
    (y..y + height).any(|row| {
        (x..x + width).any(|column| {
            let [red, green, ..] = pixel(raster, column, row);
            red > 200 && green < 150
        })
    })
}

#[test]
fn the_annotations_a_file_holds_are_read_as_marks_with_their_references() {
    let doc = annotated();
    let marks = existing_marks(&doc);
    let shapes: Vec<(u32, &str)> = marks
        .iter()
        .map(|mark| {
            let kind = match mark.shape {
                MarkShape::Rectangle(_) => "square",
                MarkShape::Oval(_) => "circle",
                MarkShape::TextBox { .. } => "text box",
                MarkShape::Highlight(_)
                | MarkShape::Underline(_)
                | MarkShape::Strikethrough(_)
                | MarkShape::Note { .. }
                | MarkShape::Line { .. }
                | MarkShape::Arrow { .. }
                | MarkShape::Ink(_) => "other",
            };
            (mark.page.0, kind)
        })
        .collect();
    assert_eq!(
        shapes,
        [(0, "square"), (0, "square"), (1, "circle"), (2, "text box")]
    );
    assert!(marks.iter().all(|mark| mark.existing.is_some()));
    assert_eq!(marks[0].color, MarkColor::Red);
    let MarkShape::Rectangle(first) = marks[0].shape else {
        panic!("a square: {:?}", marks[0].shape)
    };
    for (got, want) in [
        (first.left().0, 100),
        (first.top().0, 500),
        (first.right().0, 300),
        (first.bottom().0, 600),
    ] {
        assert!(got.abs_diff(want) <= 3, "{first:?}");
    }
    // The text box keeps its words.
    assert!(matches!(&marks[3].shape, MarkShape::TextBox { text, .. } if text == "Hello"));
}

#[test]
fn the_links_and_widgets_of_a_file_are_not_marks() {
    assert!(existing_marks(&fixture()).is_empty());
    let form = PdfDocument::from_bytes(form_bytes()).unwrap();
    assert!(existing_marks(&form).is_empty());
}

/// How many red pixels the tile of page 0 that holds the top edge of the first square has,
/// drawn the way the window draws its pages. The fixture's own page has red of its own, so a
/// render is judged against another render rather than against none.
fn tile_reds(doc: &PdfDocument) -> usize {
    let batch = TileBatch {
        page: PageIndex(0),
        zoom: ZoomBucket::containing(Permille(1000)),
        priority: Priority::Visible,
        tiles: vec![TileCoord {
            x: 0,
            y: 396 / TILE_SIDE,
        }],
    };
    let job = PdfJob::Tiles {
        ticket: Ticket(1),
        batch,
    };
    let (_, drawn, end) = tiles(run(doc, job));
    assert!(matches!(end, End::Complete), "{end:?}");
    let raster = &drawn[0].raster;
    let size = raster.size();
    (0..size.height.0)
        .flat_map(|row| (0..size.width.0).map(move |column| (column, row)))
        .filter(|&(column, row)| {
            let [red, green, ..] = pixel(raster, column, row);
            red > 200 && green < 150
        })
        .count()
}

#[test]
fn a_document_hiding_its_marks_draws_them_no_more_in_its_tiles_but_still_in_an_export() {
    let page_alone = tile_reds(&fixture());
    let doc = annotated();
    assert!(
        tile_reds(&doc) > page_alone,
        "drawn with the file's annotations"
    );
    let marks = existing_marks(&doc);
    let hidden = annotated().hiding(&marks);
    assert_eq!(
        tile_reds(&hidden),
        page_alone,
        "left out for the window to draw"
    );
    assert_ne!(
        hidden.id(),
        doc.id(),
        "a worker's prepared pages are not reused"
    );
    // An export is of the file, so it draws them all the same.
    let page = drawn(&hidden, 0, Dpi::PAGE);
    assert!(has_red(&page, 61, 394, 123, 5));
}

#[test]
fn a_save_that_touches_no_held_mark_appends_nothing_for_it() {
    let doc = annotated();
    let before = doc.bytes().to_vec();
    assert_eq!(write_markup(&doc, &MarkupSave::default()).unwrap(), before);
    // A new mark is appended after the original bytes, which stay as they were.
    let added = write_markup(
        &doc,
        &save(
            Vec::new(),
            vec![mark(MarkShape::Rectangle(rect(0, 0, 100, 100)))],
        ),
    )
    .unwrap();
    assert!(added.starts_with(&before));
    assert_eq!(annotations_on(added, 0), 3);
}

#[test]
fn a_held_mark_that_moved_is_rewritten_where_it_is_and_the_rest_stays() {
    let doc = annotated();
    let marks = existing_marks(&doc);
    let moved = marks[0].moved(100, 50);
    let bytes = write_markup(
        &doc,
        &MarkupSave {
            marks: vec![moved.clone()],
            ..MarkupSave::default()
        },
    )
    .unwrap();
    // The same number of annotations on every page.
    for (page, count) in [(0, 2), (1, 1), (2, 1)] {
        assert_eq!(annotations_on(bytes.clone(), page), count, "page {page}");
    }
    let reopened = PdfDocument::from_bytes(bytes).unwrap();
    let after = existing_marks(&reopened);
    assert_eq!(after.len(), 4);
    // It keeps its object, its colour and its size; it is where it was moved to.
    assert_eq!(after[0].existing, marks[0].existing);
    assert_eq!(after[0].color, MarkColor::Red);
    let (MarkShape::Rectangle(got), MarkShape::Rectangle(want)) = (&after[0].shape, &moved.shape)
    else {
        panic!("squares: {:?}", after[0].shape)
    };
    for (got, want) in [
        (got.left().0, want.left().0),
        (got.top().0, want.top().0),
        (got.right().0, want.right().0),
        (got.bottom().0, want.bottom().0),
    ] {
        assert!(got.abs_diff(want) <= 3, "{got:?} vs {want:?}");
    }
    // The others are where they were.
    assert_eq!(after[1].shape, marks[1].shape);
    assert_eq!(after[2].shape, marks[2].shape);
}

#[test]
fn a_held_mark_that_was_deleted_is_taken_out_and_no_other_page_changes() {
    let doc = annotated();
    let marks = existing_marks(&doc);
    let bytes = write_markup(
        &doc,
        &MarkupSave {
            removed: vec![marks[1].clone()],
            ..MarkupSave::default()
        },
    )
    .unwrap();
    for (page, count) in [(0, 1), (1, 1), (2, 1)] {
        assert_eq!(annotations_on(bytes.clone(), page), count, "page {page}");
    }
    let after = existing_marks(&PdfDocument::from_bytes(bytes).unwrap());
    assert_eq!(after.len(), 3);
    assert!(after.iter().all(|mark| mark.existing != marks[1].existing));
}

/// The fixture with one red square on its first page, written by another program: it has a name
/// (`/NM`) and an author (`/T`) that a mark does not carry.
fn named_square() -> PdfDocument {
    let base = pdfrum::Document::from_bytes(fixture_bytes()).unwrap();
    let mut edit = pdfrum_edit::EditDoc::new(base.parser());
    let square = pdfrum_edit::SquareSpec::new(
        pdfrum::Rect::new(100.0, 300.0, 250.0, 400.0),
        pdfrum::Color::from_rgb8(255, 0, 0),
    )
    .name("sq-1")
    .author("Ada");
    pdfrum_edit::add_annotation(&mut edit, 0, square).unwrap();
    let mut out = Vec::new();
    pdfrum_edit::save(
        &edit,
        &pdfrum_edit::SaveOptions::builder()
            .mode(pdfrum_edit::SaveMode::Incremental)
            .build(),
        &mut out,
    )
    .unwrap();
    PdfDocument::from_bytes(out).unwrap()
}

/// The square of page 0 of `bytes`: its rectangle, name and author.
fn square_of(bytes: Vec<u8>) -> (pdfrum::Rect, Option<String>, Option<String>) {
    let reopened = pdfrum::Document::from_bytes(bytes).unwrap();
    let page = reopened.page(0).unwrap();
    let square = page
        .annotations()
        .find(|annotation| annotation.subtype() == Subtype::Square)
        .unwrap();
    (square.rect(), square.name(), square.title())
}

#[test]
fn a_moved_held_mark_is_translated_by_exactly_the_distance_and_keeps_its_other_keys() {
    let doc = named_square();
    let marks = existing_marks(&doc);
    assert_eq!(marks.len(), 1);
    let moved = marks[0].moved(100, 50);
    let bytes = write_markup(
        &doc,
        &MarkupSave {
            marks: vec![moved],
            ..MarkupSave::default()
        },
    )
    .unwrap();
    let (rect, name, author) = square_of(bytes);
    // A tenth of the 612 pt width across and a twentieth of the 792 pt height down.
    let (dx, dy) = (61.2, -39.6);
    for (got, want) in [
        (rect.x0, 100.0 + dx),
        (rect.y0, 300.0 + dy),
        (rect.x1, 250.0 + dx),
        (rect.y1, 400.0 + dy),
    ] {
        assert!((got - want).abs() < 0.01, "{rect:?}");
    }
    assert_eq!(name.as_deref(), Some("sq-1"));
    assert_eq!(author.as_deref(), Some("Ada"));
}

#[test]
fn a_recoloured_held_mark_changes_its_colour_and_keeps_its_place_and_other_keys() {
    let doc = named_square();
    let marks = existing_marks(&doc);
    assert_eq!(marks[0].color, MarkColor::Red);
    let blue = Mark {
        color: MarkColor::Blue,
        ..marks[0].clone()
    };
    let bytes = write_markup(
        &doc,
        &MarkupSave {
            marks: vec![blue],
            ..MarkupSave::default()
        },
    )
    .unwrap();
    let after = existing_marks(&PdfDocument::from_bytes(bytes.clone()).unwrap());
    assert_eq!(after[0].color, MarkColor::Blue);
    assert_eq!(after[0].shape, marks[0].shape);
    let (rect, name, author) = square_of(bytes);
    assert!(
        (rect.x0 - 100.0).abs() < 0.01 && (rect.y1 - 400.0).abs() < 0.01,
        "{rect:?}"
    );
    assert_eq!(name.as_deref(), Some("sq-1"));
    assert_eq!(author.as_deref(), Some("Ada"));
}
