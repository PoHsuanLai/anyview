//! What is written on the open PDF across regions: what leaving, saving and undo do while fields
//! are filled and marks made, and how the pointer tool and the keys reach the markup.

use super::support::params;
use crate::command::{Command, MarkupCommand};
use crate::edits::{EditCaution, EditOffer, EditRequest, Rewind};
use crate::hand::Tool;
use crate::keys::{Act, Press};
use crate::load::{Load, Ticket};
use crate::markup::{Allowance, MarkupIn, PdfMarkup, Using};
use crate::sheet::{Departure, Sheet, SheetIn, SheetOut, SheetParams};
use crate::stage::{PdfStage, Stage};
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{
    Edit, FormWidget, MarkTool, PageIndex, PageRect, Permille, WidgetAccess, WidgetKind,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};
use std::sync::Arc;

fn field() -> FormWidget {
    FormWidget {
        name: "name".to_owned(),
        kind: WidgetKind::Text,
        access: WidgetAccess::Editable,
        page: PageIndex(0),
        rect: PageRect {
            left: Permille(100),
            top: Permille(300),
            right: Permille(400),
            bottom: Permille(340),
        },
        value: String::new(),
        on: String::new(),
        options: Vec::new(),
        exports: Vec::new(),
    }
}

/// A PDF showing, with one text field, that can be filled and marked.
fn pdf() -> Viewer {
    let (markup, _) = PdfMarkup::default().step(MarkupIn::Landed {
        widgets: Arc::from(vec![field()]),
        filling: Allowance::Granted,
        marking: Allowance::Granted,
        marks: Arc::from(Vec::new()),
    });
    Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: Stage::Pdf(PdfStage::default()),
        markup,
        ..Viewer::default()
    }
}

/// The same with the field filled in and not saved.
fn filled() -> Viewer {
    markup(
        pdf(),
        MarkupIn::Type {
            widget: 0,
            text: "Ada".to_owned(),
        },
    )
}

fn markup(viewer: Viewer, input: MarkupIn) -> Viewer {
    step(viewer, ViewerIn::Markup(input)).0
}

fn step(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &params())
}

fn step_signed(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    let signed = ViewerParams {
        sheet: SheetParams {
            edit: EditOffer::Asks(EditCaution::Signed),
            ..SheetParams::default()
        },
        ..params()
    };
    viewer.step(input, Stamp(0), &(), &signed)
}

fn the_save() -> EditRequest {
    EditRequest::on_page(Edit::Markup, PageIndex(0))
}

const CLOSE: ViewerIn = ViewerIn::Key(Press::Act(Act::Close));
const SAVE: ViewerIn = ViewerIn::Key(Press::Act(Act::Save));
const UNDO: ViewerIn = ViewerIn::Key(Press::Act(Act::Undo));

fn key(keys: &[ShortcutKey]) -> ViewerIn {
    ViewerIn::Key(Press::Key(Shortcut(keys.to_vec())))
}

#[test]
fn closing_a_pdf_with_fields_filled_asks_and_each_answer_goes_where_it_should() {
    let (asked, outs) = step(filled(), CLOSE);
    assert_eq!(asked.sheet, Sheet::Unsaved(Departure::Close));
    assert!(
        !outs.contains(&ViewerOut::CloseWindow),
        "the window waits for the answer"
    );

    let (cancelled, _) = step(asked.clone(), ViewerIn::Sheet(SheetIn::Cancel));
    assert!(cancelled.markup.is_edited(), "Cancel keeps what was filled");

    let (declined, outs) = step(asked.clone(), ViewerIn::Sheet(SheetIn::Discard));
    assert!(
        outs.contains(&ViewerOut::CloseWindow),
        "Don't Save closes at once"
    );
    assert!(!declined.markup.is_edited());

    let (saving, outs) = step(asked, ViewerIn::Sheet(SheetIn::Confirm));
    assert!(outs.contains(&ViewerOut::Edit(the_save())), "{outs:?}");
    assert!(
        !outs.contains(&ViewerOut::CloseWindow),
        "it closes once the file is read again"
    );
    assert_eq!(saving.after_save, Some(Departure::Close));
}

#[test]
fn a_pdf_with_nothing_written_closes_without_a_question_and_saves_nothing() {
    let (closed, outs) = step(pdf(), CLOSE);
    assert_eq!(closed.sheet, Sheet::Closed);
    assert!(outs.contains(&ViewerOut::CloseWindow));
    let (_, outs) = step(pdf(), SAVE);
    assert!(outs.is_empty(), "{outs:?}");
}

#[test]
fn saving_a_signed_pdf_asks_first_and_an_unsigned_one_saves_at_once() {
    let (plain, outs) = step(filled(), SAVE);
    assert_eq!(plain.sheet, Sheet::Closed);
    assert_eq!(outs, [ViewerOut::Edit(the_save())]);

    let (asked, outs) = step_signed(filled(), SAVE);
    assert_eq!(
        asked.sheet,
        Sheet::ConfirmEdit {
            request: the_save(),
            caution: EditCaution::Signed,
        }
    );
    assert!(!outs.iter().any(|out| matches!(out, ViewerOut::Edit(_))));
}

#[test]
fn undo_takes_back_what_was_written_before_it_reaches_the_file() {
    let (once, outs) = step(filled(), UNDO);
    assert!(!once.markup.is_edited());
    assert!(outs.is_empty(), "{outs:?}");
    let (_, outs) = step(once, UNDO);
    assert_eq!(outs, [ViewerOut::Rewind(Rewind::Undo)]);
}

#[test]
fn the_markup_tool_is_only_for_a_pdf_that_can_be_marked_and_the_pointer_follows_the_tool() {
    let (marking, _) = step(pdf(), ViewerIn::Run(Command::UseTool(Tool::Markup)));
    assert_eq!(
        (marking.hand.tool, marking.markup.using()),
        (Tool::Markup, Using::Marking)
    );

    // H switches between Pan and Select, and Select selects words.
    let (selecting, _) = step(pdf(), key(&[ShortcutKey::Char('h')]));
    assert_eq!(
        (selecting.hand.tool, selecting.markup.using()),
        (Tool::Select, Using::Selecting)
    );
    let (reading, _) = step(selecting, key(&[ShortcutKey::Char('h')]));
    assert_eq!(
        (reading.hand.tool, reading.markup.using()),
        (Tool::Pan, Using::Reading)
    );

    // A document that does not let the person mark has no such tool.
    let locked = Viewer {
        markup: PdfMarkup::default(),
        ..pdf()
    };
    let (still, _) = step(locked, ViewerIn::Run(Command::UseTool(Tool::Markup)));
    assert_eq!(
        (still.hand.tool, still.markup.using()),
        (Tool::Pan, Using::Reading)
    );

    // A picture has none either, and the tool does not go with the person to another file.
    let picture = Viewer {
        stage: super::support::image(),
        ..Viewer::default()
    };
    let (picture, _) = step(picture, ViewerIn::Run(Command::UseTool(Tool::Markup)));
    assert_eq!(picture.hand.tool, Tool::Pan);
    let (next, _) = step(
        marking,
        ViewerIn::Open(anyview_core::FilePath::new("/next.pdf").unwrap()),
    );
    assert_eq!(next.hand.tool, Tool::Pan);
}

#[test]
fn a_tool_chosen_with_words_selected_marks_them_and_without_turns_the_markup_tool_on() {
    let lines = vec![PageRect {
        left: Permille(100),
        top: Permille(100),
        right: Permille(400),
        bottom: Permille(120),
    }];
    let selected = markup(
        step(pdf(), key(&[ShortcutKey::Char('h')])).0,
        MarkupIn::Select(Some((PageIndex(0), lines))),
    );
    let (marked, _) = step(
        selected,
        ViewerIn::Run(Command::Markup(MarkupCommand::Tool(MarkTool::Highlight))),
    );
    assert_eq!(marked.markup.marks().len(), 1);
    assert_eq!(
        marked.hand.tool,
        Tool::Select,
        "marking words leaves the pointer as it was"
    );

    let (chosen, _) = step(
        pdf(),
        ViewerIn::Run(Command::Markup(MarkupCommand::Tool(MarkTool::Rectangle))),
    );
    assert_eq!(
        (chosen.hand.tool, chosen.markup.tool()),
        (Tool::Markup, MarkTool::Rectangle)
    );
}

#[test]
fn escape_puts_away_what_is_picked_and_delete_takes_the_picked_mark_away() {
    let drawn = [
        ViewerIn::Run(Command::Markup(MarkupCommand::Tool(MarkTool::Rectangle))),
        ViewerIn::Markup(MarkupIn::Press {
            page: PageIndex(0),
            at: anyview_core::PagePoint::at(100, 100),
            reach: 8,
        }),
        ViewerIn::Markup(MarkupIn::Drag(anyview_core::PagePoint::at(300, 200))),
        ViewerIn::Markup(MarkupIn::Release),
        ViewerIn::Run(Command::Markup(MarkupCommand::Tool(MarkTool::Pointer))),
        ViewerIn::Markup(MarkupIn::Press {
            page: PageIndex(0),
            at: anyview_core::PagePoint::at(200, 150),
            reach: 8,
        }),
        ViewerIn::Markup(MarkupIn::Release),
    ]
    .into_iter()
    .fold(pdf(), |viewer, input| step(viewer, input).0);
    assert_eq!(drawn.markup.picked(), Some(0));
    let (cleared, _) = step(drawn.clone(), key(&[ShortcutKey::Escape]));
    assert_eq!(cleared.markup.picked(), None);
    assert_eq!(cleared.markup.marks().len(), 1);
    let (deleted, _) = step(drawn, key(&[ShortcutKey::Backspace]));
    assert!(deleted.markup.marks().is_empty());
}

#[test]
fn tab_with_the_keyboard_in_a_field_goes_to_the_next_field_and_not_to_the_chrome() {
    let inside = markup(pdf(), MarkupIn::Focus(0));
    let (after, outs) = step(inside, key(&[ShortcutKey::Tab]));
    assert_eq!(
        after.markup.focus(),
        Some(0),
        "the only field wraps to itself"
    );
    assert!(
        !outs.iter().any(|out| matches!(out, ViewerOut::Chrome(_))),
        "{outs:?}"
    );
}

#[test]
fn a_page_edit_with_writing_held_writes_the_markup_first_and_then_the_pages() {
    use anyview_core::{FileAction, QuarterTurn};
    let turn = EditRequest::on_page(Edit::Rotate(QuarterTurn::Quarter), PageIndex(0));
    let rotate = ViewerIn::Run(Command::File(FileAction::RotateRight));
    // Nothing held: the turn alone.
    let (_, outs) = step(pdf(), rotate.clone());
    assert_eq!(outs, [ViewerOut::Edit(turn)]);
    // Fields filled: the markup goes first, then the turn, and the edited mark stays until the
    // file has been read again.
    let (held, outs) = step(filled(), rotate.clone());
    assert_eq!(outs, [ViewerOut::Edit(the_save()), ViewerOut::Edit(turn)]);
    assert!(held.markup.is_edited());
    // A signed document asks once; the answer writes the markup and then the turn.
    let (asked, outs) = step_signed(filled(), rotate);
    assert!(!outs.iter().any(|out| matches!(out, ViewerOut::Edit(_))));
    let (_, outs) = step_signed(asked, ViewerIn::Sheet(SheetIn::Confirm));
    let edits: Vec<&ViewerOut> = outs
        .iter()
        .filter(|out| {
            matches!(
                out,
                ViewerOut::Edit(_) | ViewerOut::Sheet(SheetOut::Edit(_))
            )
        })
        .collect();
    assert_eq!(
        edits,
        [
            &ViewerOut::Edit(the_save()),
            &ViewerOut::Sheet(SheetOut::Edit(turn))
        ]
    );
}
