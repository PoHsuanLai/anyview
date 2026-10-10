//! The context menu across regions: where it may open, what pins the chrome while it is up, and
//! what a picked row does to the regions.

use super::support::*;
use crate::chrome::{Chrome, PinReason, PinReasons};
use crate::command::Command;
use crate::context::{
    ContextEntry, ContextIn, ContextMenu, ContextParams, ContextPick, Spot, entries,
};
use crate::keys::Press;
use crate::load::{Load, Ticket};
use crate::panel::{Panel, PanelTab};
use crate::sheet::Sheet;
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{Adjust, FileAction, QuarterTurn};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

const HERE: Spot = Spot { x: 40, y: 50 };
const ROTATE: ContextPick = ContextPick::Run(Command::File(FileAction::RotateLeft));

fn rows() -> ViewerParams {
    let params = params();
    let listed = [Command::File(FileAction::RotateLeft), Command::OpenFile];
    ViewerParams {
        context: ContextParams {
            entries: entries(&listed, params.panel.tabs),
            centre: Spot { x: 450, y: 300 },
        },
        ..params
    }
}

fn showing() -> Viewer {
    Viewer {
        load: Load::Ready { ticket: Ticket(1) },
        stage: image(),
        ..Viewer::default()
    }
}

fn step(viewer: Viewer, input: ViewerIn) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(input, Stamp(0), &(), &rows())
}

fn menu_key() -> ViewerIn {
    ViewerIn::Key(Press::Key(Shortcut(vec![ShortcutKey::ContextMenu])))
}

#[test]
fn a_secondary_click_opens_the_menu_at_the_pointer_and_holds_the_chrome() {
    let (viewer, outs) = step(showing(), ViewerIn::Context(ContextIn::Open(HERE)));
    assert_eq!(viewer.context, ContextMenu::Open { at: HERE });
    assert_eq!(
        viewer.chrome,
        Chrome::Pinned {
            by: PinReasons::of(PinReason::MenuOpen)
        }
    );
    assert_eq!(outs, vec![FADE_IN]);
}

#[test]
fn closing_the_menu_lets_the_chrome_go() {
    let (viewer, _) = step(showing(), ViewerIn::Context(ContextIn::Open(HERE)));
    let (viewer, _) = step(viewer, ViewerIn::Context(ContextIn::Close));
    assert_eq!(viewer.context, ContextMenu::Closed);
    assert!(
        matches!(viewer.chrome, Chrome::Shown { .. }),
        "{:?}",
        viewer.chrome
    );
}

#[test]
fn the_menu_key_opens_it_at_the_middle_and_escape_closes_it() {
    let (viewer, _) = step(showing(), menu_key());
    assert_eq!(
        viewer.context,
        ContextMenu::Open {
            at: Spot { x: 450, y: 300 }
        }
    );
    let (viewer, _) = step(
        viewer,
        ViewerIn::Key(Press::Key(Shortcut(vec![ShortcutKey::Escape]))),
    );
    assert_eq!(viewer.context, ContextMenu::Closed);
}

#[test]
fn the_menu_does_not_open_over_a_sheet_the_palette_or_a_file_that_is_not_showing() {
    // name, the viewer
    let cases = [
        (
            "a sheet",
            Viewer {
                sheet: Sheet::ConfirmTrash,
                ..showing()
            },
        ),
        (
            "the palette",
            Viewer {
                palette: palette_on(0),
                ..showing()
            },
        ),
        ("no file yet", Viewer::default()),
    ];
    for (name, viewer) in cases {
        let (after, _) = step(viewer, ViewerIn::Context(ContextIn::Open(HERE)));
        assert_eq!(after.context, ContextMenu::Closed, "{name}");
    }
}

#[test]
fn a_picked_row_runs_as_the_palettes_row_does() {
    let editing = Viewer {
        picture: editable(),
        ..showing()
    };
    let (viewer, _) = step(editing, ViewerIn::Context(ContextIn::Open(HERE)));
    let (viewer, outs) = step(viewer, ViewerIn::Context(ContextIn::Pick(ROTATE)));
    assert_eq!(
        outs,
        vec![refitted()],
        "the turn waits for a save, and the picture fits the window again"
    );
    assert_eq!(
        viewer.picture.adjust(),
        Adjust::NONE.turned(QuarterTurn::ThreeQuarter)
    );
    // The menu fades out before it closes, so a pick leaves it up until the close arrives.
    assert_eq!(viewer.context, ContextMenu::Open { at: HERE });
}

#[test]
fn get_info_shows_the_info_tab() {
    let (viewer, _) = step(showing(), ViewerIn::Context(ContextIn::Open(HERE)));
    let (viewer, _) = step(
        viewer,
        ViewerIn::Context(ContextIn::Pick(ContextPick::GetInfo)),
    );
    assert_eq!(
        viewer.panel,
        Panel::Shown {
            tab: PanelTab::Info
        }
    );
}

#[test]
fn the_menu_lists_get_info_where_the_file_has_an_info_tab() {
    let listed = rows().context.entries;
    assert!(
        listed.contains(&ContextEntry::Item {
            pick: ContextPick::GetInfo,
            title: "Get Info"
        }),
        "{listed:?}"
    );
}

#[test]
fn the_open_row_asks_for_a_file_as_ctrl_o_does() {
    let (viewer, _) = step(showing(), ViewerIn::Context(ContextIn::Open(HERE)));
    let (_, outs) = step(
        viewer,
        ViewerIn::Context(ContextIn::Pick(ContextPick::Run(Command::OpenFile))),
    );
    assert_eq!(outs, vec![ViewerOut::PickFile]);
}
