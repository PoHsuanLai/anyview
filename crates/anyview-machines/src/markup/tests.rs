//! The markup as tables: the order the keyboard walks the fields in, what typing and clicking do
//! to their values, and what each tool makes.

use super::{Allowance, MarkupIn, MarkupOut, PdfMarkup, Using};
use anyview_core::{
    FormWidget, Mark, MarkColor, MarkShape, MarkTool, PageIndex, PagePoint, PageRect, Permille,
    WidgetAccess, WidgetKind,
};
use std::sync::Arc;

fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
    PageRect {
        left: Permille(left),
        top: Permille(top),
        right: Permille(right),
        bottom: Permille(bottom),
    }
}

fn widget(name: &str, kind: WidgetKind, page: u32, value: &str) -> FormWidget {
    FormWidget {
        name: name.to_owned(),
        kind,
        access: WidgetAccess::Editable,
        page: PageIndex(page),
        rect: rect(100, 300, 400, 340),
        value: value.to_owned(),
        on: if kind.is_toggle() {
            "Yes".to_owned()
        } else {
            String::new()
        },
        options: Vec::new(),
        exports: Vec::new(),
    }
}

/// Fields: 0 name (page 0), 1 agree (page 0), 2 locked (page 0, read only), 3 colour (page 1),
/// 4 and 5 two radio buttons of one group (page 1, states `A` and `B`).
fn widgets() -> Vec<FormWidget> {
    let mut locked = widget("locked", WidgetKind::Text, 0, "fixed");
    locked.access = WidgetAccess::ReadOnly;
    let mut a = widget("pick", WidgetKind::Radio, 1, "Off");
    a.on = "A".to_owned();
    let mut b = widget("pick", WidgetKind::Radio, 1, "Off");
    b.on = "B".to_owned();
    vec![
        widget("name", WidgetKind::Text, 0, ""),
        widget("agree", WidgetKind::Check, 0, "Off"),
        locked,
        widget("colour", WidgetKind::Choice, 1, "Red"),
        a,
        b,
    ]
}

fn landed(filling: Allowance, marking: Allowance) -> PdfMarkup {
    PdfMarkup::default()
        .step(MarkupIn::Landed {
            widgets: Arc::from(widgets()),
            filling,
            marking,
        })
        .0
}

fn open() -> PdfMarkup {
    landed(Allowance::Granted, Allowance::Granted)
}

fn marking() -> PdfMarkup {
    run(open(), [MarkupIn::Using(Using::Marking)])
}

fn run(markup: PdfMarkup, inputs: impl IntoIterator<Item = MarkupIn>) -> PdfMarkup {
    inputs
        .into_iter()
        .fold(markup, |markup, input| markup.step(input).0)
}

fn point(x: u32, y: u32) -> PagePoint {
    PagePoint::at(x, y)
}

fn press(x: u32, y: u32) -> MarkupIn {
    MarkupIn::Press {
        page: PageIndex(0),
        at: point(x, y),
        reach: 8,
    }
}

fn text(widget: usize, text: &str) -> MarkupIn {
    MarkupIn::Type {
        widget,
        text: text.to_owned(),
    }
}

#[test]
fn tab_walks_the_fields_that_take_a_value_in_the_documents_order_and_wraps() {
    // name, the steps, the focus after them
    let cases: &[(&str, &[MarkupIn], Option<usize>)] = &[
        (
            "first tab goes to the first field",
            &[MarkupIn::Next],
            Some(0),
        ),
        (
            "then on, skipping the read-only one",
            &[MarkupIn::Next, MarkupIn::Next, MarkupIn::Next],
            Some(3),
        ),
        (
            "past the last it wraps to the first",
            &[
                MarkupIn::Next,
                MarkupIn::Next,
                MarkupIn::Next,
                MarkupIn::Next,
                MarkupIn::Next,
                MarkupIn::Next,
            ],
            Some(0),
        ),
        (
            "shift tab from nothing goes to the last",
            &[MarkupIn::Previous],
            Some(5),
        ),
        (
            "shift tab goes back, skipping the read-only one",
            &[MarkupIn::Focus(3), MarkupIn::Previous],
            Some(1),
        ),
        (
            "a press puts the keyboard in a field and a read-only one refuses it",
            &[MarkupIn::Focus(1), MarkupIn::Focus(2)],
            Some(1),
        ),
        (
            "leaving a field it was not in changes nothing",
            &[MarkupIn::Focus(1), MarkupIn::Blur(0)],
            Some(1),
        ),
        (
            "leaving the field it is in puts it nowhere",
            &[MarkupIn::Focus(1), MarkupIn::Blur(1)],
            None,
        ),
    ];
    for (name, steps, focus) in cases {
        let markup = run(open(), steps.iter().cloned());
        assert_eq!(markup.focus(), *focus, "{name}");
    }
}

#[test]
fn tab_shows_the_page_of_a_field_only_when_it_is_on_another() {
    let (markup, outs) = open().step(MarkupIn::Next);
    assert_eq!(
        outs,
        [MarkupOut::Reveal {
            page: PageIndex(0),
            offset: Permille(200)
        }]
    );
    let (_, outs) = markup.step(MarkupIn::Next);
    assert!(outs.is_empty(), "the next field is on the same page");
    let (_, outs) = run(open(), [MarkupIn::Focus(1)]).step(MarkupIn::Next);
    assert!(
        matches!(outs.as_slice(), [MarkupOut::Reveal { page, .. }] if *page == PageIndex(1)),
        "{outs:?}"
    );
}

#[test]
fn a_document_that_cannot_be_filled_takes_no_input() {
    let closed = landed(Allowance::Withheld, Allowance::Withheld);
    let after = run(
        closed,
        [
            MarkupIn::Next,
            MarkupIn::Focus(0),
            text(0, "x"),
            MarkupIn::Toggle(1),
            MarkupIn::Using(Using::Marking),
            MarkupIn::Tool(MarkTool::Rectangle),
            press(100, 100),
            MarkupIn::Drag(point(300, 300)),
            MarkupIn::Release,
        ],
    );
    assert!(!after.takes_input(0) && !after.can_mark());
    assert_eq!(
        (after.focus(), after.is_edited(), after.using()),
        (None, false, Using::Reading)
    );
}

#[test]
fn typing_clicking_and_choosing_change_the_values_and_go_back_together() {
    let markup = open();
    let value =
        |markup: &PdfMarkup, widget: usize| markup.value_of(&markup.widgets()[widget]).to_owned();
    // Four letters in one field are one step to take back.
    let typed = run(
        markup.clone(),
        [
            text(0, "A"),
            text(0, "Ad"),
            text(0, "Ada"),
            text(0, "Ada L"),
        ],
    );
    assert_eq!(value(&typed, 0), "Ada L");
    assert!(typed.is_edited());
    assert_eq!(value(&run(typed.clone(), [MarkupIn::Undo]), 0), "");
    assert!(!run(typed.clone(), [MarkupIn::Undo]).is_edited());
    assert_eq!(
        value(&run(typed, [MarkupIn::Undo, MarkupIn::Redo]), 0),
        "Ada L"
    );
    // A check box flips, and a click on a radio button sets its group to that button.
    let toggled = run(markup.clone(), [MarkupIn::Toggle(1)]);
    assert!(toggled.is_on(1));
    assert!(!run(toggled, [MarkupIn::Toggle(1)]).is_on(1));
    let radios = run(markup.clone(), [MarkupIn::Toggle(4), MarkupIn::Toggle(5)]);
    assert_eq!((radios.is_on(4), radios.is_on(5)), (false, true));
    let again = run(radios, [MarkupIn::Toggle(5)]);
    assert!(
        again.is_on(5),
        "a radio button is not cleared by a click on it"
    );
    // A choice, then the value the file already holds: nothing is left to save.
    let chosen = run(markup.clone(), [text(3, "Green")]);
    assert_eq!(chosen.save().fills.len(), 1);
    let back = run(chosen, [text(3, "Red")]);
    assert!(!back.is_edited());
    // Typing into a read-only field, or a check box, does nothing.
    assert!(!run(markup, [text(2, "x"), text(1, "x")]).is_edited());
}

#[test]
fn what_was_written_is_saved_by_field_name() {
    let markup = run(
        open(),
        [text(0, "Ada"), MarkupIn::Toggle(1), text(3, "Blue")],
    );
    let fills: Vec<(String, String)> = markup
        .save()
        .fills
        .into_iter()
        .map(|fill| (fill.name, fill.value))
        .collect();
    assert_eq!(
        fills,
        [
            ("agree".to_owned(), "Yes".to_owned()),
            ("colour".to_owned(), "Blue".to_owned()),
            ("name".to_owned(), "Ada".to_owned()),
        ]
    );
}

fn kinds(markup: &PdfMarkup) -> Vec<&MarkShape> {
    markup.marks().iter().map(|mark| &mark.shape).collect()
}

#[test]
fn a_drag_with_a_tool_draws_its_shape_and_a_small_one_draws_nothing() {
    // name, the tool, the drag from and to, the shape it makes
    let cases = [
        (
            "rectangle",
            MarkTool::Rectangle,
            (100, 100, 300, 200),
            Some(MarkShape::Rectangle(rect(100, 100, 300, 200))),
        ),
        (
            "oval drawn from the far corner",
            MarkTool::Oval,
            (300, 200, 100, 100),
            Some(MarkShape::Oval(rect(100, 100, 300, 200))),
        ),
        (
            "line",
            MarkTool::Line,
            (100, 100, 300, 120),
            Some(MarkShape::Line {
                from: point(100, 100),
                to: point(300, 120),
            }),
        ),
        (
            "arrow",
            MarkTool::Arrow,
            (100, 100, 300, 120),
            Some(MarkShape::Arrow {
                from: point(100, 100),
                to: point(300, 120),
            }),
        ),
        (
            "a speck of a rectangle",
            MarkTool::Rectangle,
            (100, 100, 104, 103),
            None,
        ),
        (
            "a click with the line tool",
            MarkTool::Line,
            (100, 100, 100, 100),
            None,
        ),
    ];
    for (name, tool, (x0, y0, x1, y1), shape) in cases {
        let drawn = run(
            marking(),
            [
                MarkupIn::Tool(tool),
                press(x0, y0),
                MarkupIn::Drag(point(x1, y1)),
                MarkupIn::Release,
            ],
        );
        let got: Vec<&MarkShape> = kinds(&drawn);
        assert_eq!(got, shape.iter().collect::<Vec<_>>(), "{name}");
    }
}

#[test]
fn the_shape_is_a_draft_until_the_pointer_is_let_go() {
    let markup = run(
        marking(),
        [
            MarkupIn::Tool(MarkTool::Rectangle),
            press(100, 100),
            MarkupIn::Drag(point(300, 200)),
        ],
    );
    let draft = markup.draft().expect("a draft");
    assert_eq!(
        (draft.from, draft.to, draft.tool),
        (point(100, 100), point(300, 200), MarkTool::Rectangle)
    );
    assert!(markup.marks().is_empty());
    let done = run(markup, [MarkupIn::Release]);
    assert!(done.draft().is_none());
    assert_eq!(done.marks().len(), 1);
}

#[test]
fn selected_words_are_marked_in_the_colour_chosen_and_yellow_is_the_first() {
    let lines = vec![rect(100, 100, 400, 120), rect(100, 130, 250, 150)];
    let select = MarkupIn::Select(Some((PageIndex(0), lines.clone())));
    let yellow = run(
        marking(),
        [select.clone(), MarkupIn::Apply(MarkTool::Highlight)],
    );
    assert_eq!(
        yellow.marks(),
        [Mark {
            page: PageIndex(0),
            color: MarkColor::Yellow,
            shape: MarkShape::Highlight(lines.clone()),
        }]
    );
    assert!(!yellow.has_selection(), "the selection is used up");
    let green = run(
        marking(),
        [
            MarkupIn::Color(MarkColor::Green),
            select.clone(),
            MarkupIn::Apply(MarkTool::Underline),
        ],
    );
    assert_eq!(green.marks()[0].color, MarkColor::Green);
    assert_eq!(green.marks()[0].shape, MarkShape::Underline(lines));
    // Nothing selected, nothing marked; and only a text tool marks words.
    assert!(
        run(marking(), [MarkupIn::Apply(MarkTool::Highlight)])
            .marks()
            .is_empty()
    );
    let kept = run(marking(), [select, MarkupIn::Apply(MarkTool::Rectangle)]);
    assert!(kept.marks().is_empty() && kept.has_selection());
    assert!(!run(open(), [MarkupIn::Select(Some((PageIndex(0), Vec::new())))]).has_selection());
}

#[test]
fn a_note_is_written_where_the_page_is_pressed_and_goes_if_left_empty() {
    let made = run(marking(), [MarkupIn::Tool(MarkTool::Note), press(500, 400)]);
    assert_eq!(made.writing(), Some(0));
    let typed = run(
        made.clone(),
        [MarkupIn::Words("Check".to_owned()), MarkupIn::Done],
    );
    assert_eq!(typed.writing(), None);
    assert_eq!(typed.words_of(0), Some("Check"));
    assert!(typed.can_undo());
    // Left empty it is taken back, and there is nothing to undo.
    let empty = run(made, [MarkupIn::Done]);
    assert!(empty.marks().is_empty() && !empty.can_undo() && !empty.is_edited());
    // A note pressed at the edge keeps its icon on the page.
    let edge = run(
        marking(),
        [MarkupIn::Tool(MarkTool::Note), press(1000, 1000)],
    );
    let bounds = edge.marks()[0].bounds();
    assert!(
        bounds.right.0 <= 1000 && bounds.bottom.0 <= 1000,
        "{bounds:?}"
    );
}

#[test]
fn a_text_box_pressed_without_a_drag_gets_a_box_of_its_own_size() {
    let made = run(
        marking(),
        [
            MarkupIn::Tool(MarkTool::TextBox),
            press(100, 100),
            MarkupIn::Release,
            MarkupIn::Words("Hello".to_owned()),
        ],
    );
    assert_eq!(made.writing(), Some(0));
    assert_eq!(made.words_of(0), Some("Hello"));
    let bounds = made.marks()[0].bounds();
    assert_eq!((bounds.width(), bounds.height()), (250, 40));
}

#[test]
fn a_mark_is_picked_moved_recoloured_and_deleted() {
    let drawn = run(
        marking(),
        [
            MarkupIn::Tool(MarkTool::Rectangle),
            press(100, 100),
            MarkupIn::Drag(point(300, 200)),
            MarkupIn::Release,
            MarkupIn::Tool(MarkTool::Pointer),
        ],
    );
    // A press off the mark picks nothing; on it picks it.
    assert_eq!(run(drawn.clone(), [press(700, 700)]).picked(), None);
    let held = run(drawn.clone(), [press(200, 150)]);
    assert_eq!(held.picked(), Some(0));
    // Dragging moves it with the pointer, and letting go is one step.
    let moved = run(held, [MarkupIn::Drag(point(250, 250)), MarkupIn::Release]);
    assert_eq!(
        moved.marks()[0].shape,
        MarkShape::Rectangle(rect(150, 200, 350, 300))
    );
    let undone = run(moved.clone(), [MarkupIn::Undo]);
    assert_eq!(
        undone.marks()[0].shape,
        MarkShape::Rectangle(rect(100, 100, 300, 200))
    );
    // The colour chosen goes to the mark that is picked.
    let blue = run(moved.clone(), [MarkupIn::Color(MarkColor::Blue)]);
    assert_eq!(blue.marks()[0].color, MarkColor::Blue);
    // Delete takes it away, and undo brings it back.
    let gone = run(moved, [MarkupIn::Delete]);
    assert!(gone.marks().is_empty() && gone.picked().is_none());
    assert_eq!(run(gone, [MarkupIn::Undo]).marks().len(), 1);
    // A press that does not move the mark is not a step.
    let pressed = run(drawn, [press(200, 150), MarkupIn::Release]);
    assert_eq!(pressed.undo.len(), 1);
}

#[test]
fn leaving_the_markup_tool_puts_away_what_was_half_done() {
    let markup = run(
        marking(),
        [
            MarkupIn::Select(Some((PageIndex(0), vec![rect(0, 0, 100, 20)]))),
            MarkupIn::Using(Using::Selecting),
        ],
    );
    assert!(!markup.has_selection());
    let drawing = run(
        marking(),
        [
            MarkupIn::Tool(MarkTool::Rectangle),
            press(100, 100),
            MarkupIn::Using(Using::Reading),
        ],
    );
    assert!(drawing.draft().is_none() && drawing.marks().is_empty());
}
