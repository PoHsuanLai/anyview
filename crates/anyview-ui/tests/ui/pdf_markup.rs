//! Writing on a PDF in the viewer window under the harness: a field of its form is pressed, typed
//! in and left with Tab, words are highlighted with the Markup tool, and ⌘S asks the host to save
//! what was written. The window never writes the file: it says what to save.

use crate::pdf_fixture;
use crate::support;

use anyview_core::{
    FieldFill, Mark, MarkColor, MarkShape, MarkupSave, PageIndex, PageRect, Permille,
};
use anyview_pdf::{PdfDocument, write_markup};
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

/// Markup with the Pointer, as a person reaches it: a drawing tool from the palette brings up the
/// dock, and the dock's first choice is the Pointer.
fn with_the_pointer(harness: &mut Harness) {
    from_the_palette(harness, "rectangle");
    settle(harness);
    let pointer = harness
        .centre(".viewer-markup-bar .ds-segmented-segment")
        .expect("the dock's Pointer");
    harness.send(Input::click(pointer));
    settle(harness);
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

/// A drop-down reached with Tab takes the keyboard with its menu closed, and an arrow opens it; one
/// pressed opens its menu at once. Preview's form fields do both.
#[test]
fn tab_reaches_a_drop_down_closed_and_a_press_opens_it() {
    let (_dir, mut harness, _requests) = written("form.pdf", pdf_fixture::form_bytes());
    let name = harness
        .centre(".viewer-field[data-kind=text]")
        .expect("the text field");
    harness.send(Input::click(name));
    settle(&mut harness);
    // The text field, the check box, then the drop-down.
    press(&mut harness, ShortcutKey::Tab);
    press(&mut harness, ShortcutKey::Tab);
    assert_eq!(
        harness
            .attr(".viewer-field[data-state=held]", "data-kind")
            .as_deref(),
        Some("choice"),
        "Tab goes on to the drop-down"
    );
    assert_eq!(harness.count(".ds-menu"), 0, "with its menu closed");
    press(&mut harness, ShortcutKey::Down);
    assert_eq!(harness.count(".ds-menu"), 1, "an arrow opens it");
    press(&mut harness, ShortcutKey::Escape);
    settle(&mut harness);
    assert_eq!(harness.count(".ds-menu"), 0);
    // A press on the drop-down, from no field, opens it at once.
    press(&mut harness, ShortcutKey::Escape);
    settle(&mut harness);
    let choice = harness
        .centre(".viewer-field[data-kind=choice]")
        .expect("the drop-down");
    harness.send(Input::click(choice));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-menu"), 1, "a press opens the menu");
}

/// Sketch draws what the pointer is dragged through, as Preview's does: one stroke over the page,
/// saved as an ink mark through the points dragged over.
#[test]
fn a_sketch_dragged_over_the_page_is_drawn_and_saved_as_ink() {
    let (_dir, mut harness, requests) = written("plain.pdf", pdf_fixture::fixture_bytes());
    from_the_palette(&mut harness, "sketch");
    let page = harness.rect(".viewer-pdf-page").expect("the first page");
    let at = |across: f32, down: f32| Point {
        x: Px(page.origin.x.0 + page.size.width.0 * across),
        y: Px(page.origin.y.0 + page.size.height.0 * down),
    };
    harness.send(Input::drag(at(0.3, 0.4), at(0.6, 0.5), 8));
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-pdf-sketch polyline"),
        1,
        "the stroke is drawn over the page"
    );
    let points = harness
        .attr(".viewer-pdf-sketch polyline", "points")
        .unwrap_or_default();
    assert!(
        points.split(' ').count() >= 2,
        "through the points dragged over: {points:?}"
    );
    // And it is painted: the middle of the stroke is the mark's colour, not the white of the page.
    let middle = at(0.45, 0.45);
    let shot = harness.render().expect("the window renders");
    let ink = shot.get_pixel(middle.x.0 as u32, middle.y.0 as u32).0;
    assert!(
        u32::from(ink[0]) + u32::from(ink[1]) + u32::from(ink[2]) < 600,
        "the stroke is painted at {middle:?}: {ink:?}"
    );
    chord(&mut harness, 's');
    let asked = saves(&requests);
    assert_eq!(asked.len(), 1);
    let [mark] = asked[0].marks.as_slice() else {
        panic!("one mark: {:?}", asked[0].marks)
    };
    let MarkShape::Ink(strokes) = &mark.shape else {
        panic!("an ink mark: {mark:?}")
    };
    assert_eq!(strokes.len(), 1, "one stroke");
    assert!(strokes[0].len() >= 2, "{strokes:?}");
}

/// The fixture with a red square on its first page, right of the page's own red box,
/// as a file another program might have left it.
fn with_a_square() -> Vec<u8> {
    let square = Mark {
        page: PageIndex(0),
        color: MarkColor::Red,
        shape: MarkShape::Rectangle(PageRect {
            left: Permille(550),
            top: Permille(300),
            right: Permille(800),
            bottom: Permille(450),
        }),
        existing: None,
    };
    let doc = PdfDocument::from_bytes(pdf_fixture::fixture_bytes()).unwrap();
    let save = MarkupSave {
        marks: vec![square],
        ..MarkupSave::default()
    };
    write_markup(&doc, &save).unwrap()
}

/// Whether the window, rendered, has a red pixel within a few pixels either side of `at`.
fn red_near(harness: &mut Harness, at: Point) -> bool {
    let shot = harness.render().expect("the window renders");
    let (x, y) = (at.x.0 as u32, at.y.0 as u32);
    (x.saturating_sub(4)..=x + 4).any(|column| {
        let [red, green, ..] = shot.get_pixel(column, y).0;
        red > 200 && green < 150
    })
}

#[test]
fn a_square_the_file_holds_is_drawn_once_and_a_press_then_delete_takes_it_away() {
    let (_dir, mut harness, requests) = written("square.pdf", with_a_square());
    let page = harness.rect(".viewer-pdf-page").expect("the first page");
    let at = |across: f32, down: f32| Point {
        x: Px(page.origin.x.0 + page.size.width.0 * across),
        y: Px(page.origin.y.0 + page.size.height.0 * down),
    };
    assert_eq!(
        harness.count(".viewer-pdf-ink[data-ink=frame]"),
        1,
        "the window draws the file's square as a mark"
    );
    let edge = at(0.55, 0.375);
    assert!(red_near(&mut harness, edge), "the square is painted");
    // With the pointer tool, a press inside picks it, and Delete takes it away.
    with_the_pointer(&mut harness);
    harness.send(Input::click(at(0.675, 0.375)));
    settle(&mut harness);

    assert_eq!(
        harness.count(".viewer-pdf-ink[data-state=picked]"),
        1,
        "the square is picked"
    );
    press(&mut harness, ShortcutKey::Delete);
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-pdf-ink[data-ink=frame]"), 0);
    assert!(
        !red_near(&mut harness, edge),
        "the page is not drawn with the square under it"
    );
    chord(&mut harness, 's');
    let asked = saves(&requests);
    assert_eq!(asked.len(), 1);
    assert!(asked[0].marks.is_empty(), "{:?}", asked[0].marks);
    let [gone] = asked[0].removed.as_slice() else {
        panic!("one removed: {:?}", asked[0].removed)
    };
    assert!(gone.existing.is_some(), "{gone:?}");
}

#[test]
fn a_square_the_file_holds_is_dragged_and_saved_as_moved_with_its_reference() {
    let (_dir, mut harness, requests) = written("square.pdf", with_a_square());
    let page = harness.rect(".viewer-pdf-page").expect("the first page");
    let at = |across: f32, down: f32| Point {
        x: Px(page.origin.x.0 + page.size.width.0 * across),
        y: Px(page.origin.y.0 + page.size.height.0 * down),
    };
    with_the_pointer(&mut harness);
    harness.send(Input::drag(at(0.675, 0.375), at(0.775, 0.375), 8));
    settle(&mut harness);
    chord(&mut harness, 's');
    let asked = saves(&requests);
    assert_eq!(asked.len(), 1);
    let [moved] = asked[0].marks.as_slice() else {
        panic!("one mark: {:?}", asked[0].marks)
    };
    assert!(moved.existing.is_some(), "{moved:?}");
    let MarkShape::Rectangle(rect) = moved.shape else {
        panic!("a square: {moved:?}")
    };
    assert!(rect.left.0 > 600, "moved to the right: {rect:?}");
    assert!(asked[0].removed.is_empty());
}
