//! Writing on a PDF in the viewer window under the harness: a field of its form is pressed, typed
//! in and left with Tab, words are highlighted with the Markup tool, and ⌘S asks the host to save
//! what was written. The window never writes the file: it says what to save.

use crate::pdf_fixture;
use crate::support;

use anyview_core::{FieldFill, MarkShape, MarkupSave, PageRect};
use anyview_ui::HostRequest;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use support::{Requests, chord, press, settle, type_text, window};

fn written(name: &str, bytes: Vec<u8>) -> (tempfile::TempDir, Harness, Requests) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    let paths: Vec<PathBuf> = vec![std::fs::canonicalize(path).unwrap()];
    let (harness, requests) = window(&paths, 0, Appearance::default());
    (dir, harness, requests)
}

/// What the window asked the host to save of what was written.
fn saves(requests: &Requests) -> Vec<MarkupSave> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::SaveMarkup(save) = request {
                Some(save.clone())
            } else {
                None
            }
        })
        .collect()
}

fn from_the_palette(harness: &mut Harness, words: &str) {
    chord(harness, 'k');
    type_text(harness, words);
    press(harness, ShortcutKey::Enter);
}

#[test]
fn a_form_field_is_pressed_typed_in_left_with_tab_and_saved_with_what_was_written() {
    let (_dir, mut harness, requests) = written("form.pdf", pdf_fixture::form_bytes());
    assert_eq!(
        harness.count(".viewer-field"),
        3,
        "the text field, the check box and the drop-down are over the page"
    );
    assert_eq!(
        harness.count(".viewer-field[data-state=idle]"),
        3,
        "while the document draws them they only wait to be pressed"
    );
    // A press puts the keyboard in the field, and what is typed is written into it.
    let field = harness
        .centre(".viewer-field[data-kind=text]")
        .expect("the text field");
    harness.send(Input::click(field));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-field[data-state=held]"), 1);
    type_text(&mut harness, "Ada");
    // Tab goes to the next field, the check box, and a click on it ticks it.
    press(&mut harness, ShortcutKey::Tab);
    assert_eq!(
        harness
            .attr(".viewer-field[data-state=held]", "data-kind")
            .as_deref(),
        Some("check"),
        "the keyboard is in the next field of the document"
    );
    let tick = harness
        .centre(".viewer-field-toggle")
        .expect("the check box is live");
    harness.send(Input::click(tick));
    settle(&mut harness);
    // Save asks the host to write both values.
    chord(&mut harness, 's');
    let asked = saves(&requests);
    assert_eq!(asked.len(), 1, "{:?}", requests.lock().unwrap());
    assert_eq!(
        asked[0].fills,
        [
            FieldFill {
                name: "agree".to_owned(),
                value: "Yes".to_owned(),
            },
            FieldFill {
                name: "name".to_owned(),
                value: "Ada".to_owned(),
            },
        ]
    );
}

#[test]
fn a_page_without_a_form_shows_no_field() {
    let (_dir, harness, _) = written("plain.pdf", pdf_fixture::fixture_bytes());
    assert_eq!(harness.count(".viewer-field"), 0);
}

#[test]
fn words_dragged_over_with_the_highlighter_are_highlighted_and_saved_as_a_mark() {
    let (_dir, mut harness, requests) = written("plain.pdf", pdf_fixture::fixture_bytes());
    assert_eq!(
        harness.count(".viewer-markup-dock"),
        0,
        "no bar until the tool is on"
    );
    from_the_palette(&mut harness, "highlight");
    assert_eq!(
        harness.count(".viewer-markup-dock"),
        1,
        "the Markup tool is on"
    );
    // The sentence on the first page runs from a fifth of the way across at a sixth of the way down.
    let page = harness.rect(".viewer-pdf-page").expect("the first page");
    let at = |across: f32, down: f32| Point {
        x: Px(page.origin.x.0 + page.size.width.0 * across),
        y: Px(page.origin.y.0 + page.size.height.0 * down),
    };
    harness.send(Input::drag(at(0.13, 0.172), at(0.5, 0.172), 8));
    settle(&mut harness);
    assert!(
        harness.count(".viewer-pdf-ink[data-ink=fill]") >= 1,
        "the words are highlighted over the page"
    );
    chord(&mut harness, 's');
    let asked = saves(&requests);
    assert_eq!(asked.len(), 1);
    let [mark] = asked[0].marks.as_slice() else {
        panic!("one mark: {:?}", asked[0].marks)
    };
    let MarkShape::Highlight(lines) = &mark.shape else {
        panic!("a highlight: {mark:?}")
    };
    let line: PageRect = lines[0];
    assert!(line.left.0 < 200 && line.right.0 > 300, "{line:?}");
}
