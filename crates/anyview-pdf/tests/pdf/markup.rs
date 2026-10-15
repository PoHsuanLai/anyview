//! What a person writes on a PDF, saved and read back: form values, and marks as standard
//! annotations.

use crate::support::{fixture, form_bytes, inherited_choice_bytes};

use anyview_core::{
    FieldFill, Mark, MarkColor, MarkShape, MarkupSave, PageIndex, PagePoint, PageRect, Permille,
    WidgetAccess, WidgetKind,
};
use anyview_pdf::{PdfDocument, PdfError, form_widgets, page_words, write_markup};
use pdfrum::Subtype;

fn fill(name: &str, value: &str) -> FieldFill {
    FieldFill {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
    PageRect {
        left: Permille(left),
        top: Permille(top),
        right: Permille(right),
        bottom: Permille(bottom),
    }
}

fn mark(shape: MarkShape) -> Mark {
    Mark {
        page: PageIndex(0),
        color: MarkColor::Yellow,
        shape,
    }
}

fn save(fills: Vec<FieldFill>, marks: Vec<Mark>) -> MarkupSave {
    MarkupSave { fills, marks }
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
    assert_eq!((name.left.0, name.right.0), (118, 444));
    assert!(name.top.0 > 200 && name.top.0 < 300, "{name:?}");
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
        (shown.left.0, first.left.0),
        (shown.top.0, first.top.0),
        (shown.right.0, first.right.0),
        (shown.bottom.0, first.bottom.0),
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
