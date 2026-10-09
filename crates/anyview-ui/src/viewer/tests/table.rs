use super::support::*;
use crate::chrome::*;
use crate::command::Command;
use crate::edits::EditRequest;
use crate::load::*;
use crate::palette::*;
use crate::panel::*;
use crate::presentation::*;
use crate::sheet::*;
use crate::stage::*;
use crate::viewer::*;
use anyview_core::*;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

/// Name, state before, input, time, state after, outputs.
type Case = (
    &'static str,
    fn() -> Viewer,
    fn() -> ViewerIn,
    u64,
    fn() -> Viewer,
    fn() -> Vec<ViewerOut>,
);

const CASES: &[Case] = &[
    (
        "the pointer moving reveals the chrome",
        Viewer::default,
        || ViewerIn::Chrome(ChromeIn::PointerMoved(Zone::Content)),
        100,
        || Viewer {
            chrome: Chrome::Revealing {
                since: Stamp(100),
                until: Stamp(250),
            },
            ..Viewer::default()
        },
        || vec![FADE_IN],
    ),
    (
        "toggling the panel opens the first tab the file has",
        Viewer::default,
        || ViewerIn::Panel(PanelIn::Toggle),
        0,
        || Viewer {
            panel: Panel::Shown {
                tab: PanelTab::Info,
            },
            ..Viewer::default()
        },
        || vec![ViewerOut::Panel(PanelOut::Show(PanelTab::Info))],
    ),
    (
        "opening the palette holds the chrome up",
        Viewer::default,
        || ViewerIn::Palette(PaletteIn::Open),
        0,
        || Viewer {
            palette: palette_on(0),
            chrome: menu_pinned(),
            ..Viewer::default()
        },
        || vec![ViewerOut::Palette(PaletteOut::Opened), FADE_IN],
    ),
    (
        "a palette row for export opens the export sheet on the image's first kind",
        || Viewer {
            palette: palette_on(0),
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        0,
        || Viewer {
            sheet: Sheet::Export {
                draft: png(),
                span: crate::sheet::PageSpan::All,
            },
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Sheet(SheetOut::Opened),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "a palette row for the trash asks first",
        || Viewer {
            palette: palette_on(1),
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        0,
        || Viewer {
            sheet: Sheet::ConfirmTrash,
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Sheet(SheetOut::Opened),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "a command from a control asks the host to save the turn, like the palette's row does",
        || Viewer {
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Run(Command::File(FileAction::RotateRight)),
        0,
        || Viewer {
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![ViewerOut::Edit(EditRequest::of_picture(Edit::Rotate(
                QuarterTurn::Quarter,
            )))]
        },
    ),
    (
        "a palette row for export on a file with no stage does nothing but close",
        || Viewer {
            palette: palette_on(0),
            chrome: menu_pinned(),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        500,
        || Viewer {
            chrome: Chrome::Shown {
                idle_from: Stamp(500),
                hide_at: Stamp(2500),
            },
            ..Viewer::default()
        },
        || vec![ViewerOut::Palette(PaletteOut::Closed)],
    ),
    (
        "a palette stage command reaches the showing stage and releases the pause pin",
        || Viewer {
            palette: palette_on(2),
            chrome: Chrome::Pinned {
                by: PinReasons::of(PinReason::MenuOpen).with(PinReason::MediaPaused),
            },
            stage: media(Pace::Paused),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        1000,
        || Viewer {
            chrome: Chrome::Shown {
                idle_from: Stamp(1000),
                hide_at: Stamp(3000),
            },
            stage: media(Pace::Playing),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Stage(StageOut::Media(MediaOut::Command(
                    PlayerCommand::SetPlayback(Pace::Playing),
                ))),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "a palette turn on an image asks the host to save the turn",
        || Viewer {
            palette: palette_on(3),
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        1000,
        || Viewer {
            chrome: Chrome::Shown {
                idle_from: Stamp(1000),
                hide_at: Stamp(3000),
            },
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Edit(EditRequest::of_picture(Edit::Rotate(QuarterTurn::Quarter))),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "a palette row for the mini window shrinks a media window",
        || Viewer {
            palette: palette_on(4),
            chrome: menu_pinned(),
            stage: media(Pace::Playing),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        1000,
        || Viewer {
            chrome: Chrome::Shown {
                idle_from: Stamp(1000),
                hide_at: Stamp(3000),
            },
            presentation: Presentation::Mini,
            stage: media(Pace::Playing),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Presentation(PresentationOut::Become(Presentation::Mini)),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "a palette action with nothing for the viewer to decide goes to the edge",
        || Viewer {
            palette: palette_on(5),
            chrome: menu_pinned(),
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Palette(PaletteIn::Enter),
        1000,
        || Viewer {
            chrome: Chrome::Shown {
                idle_from: Stamp(1000),
                hide_at: Stamp(3000),
            },
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Run(FileAction::Share),
                ViewerOut::Palette(PaletteOut::Closed),
            ]
        },
    ),
    (
        "the player pausing pins the chrome up",
        || Viewer {
            stage: media(Pace::Playing),
            ..Viewer::default()
        },
        || {
            ViewerIn::Stage(StageIn::Media(MediaIn::Player(PlayerEvent::Playback(
                Pace::Paused,
            ))))
        },
        0,
        || Viewer {
            chrome: Chrome::Pinned {
                by: PinReasons::of(PinReason::MediaPaused),
            },
            stage: media(Pace::Paused),
            ..Viewer::default()
        },
        || vec![FADE_IN],
    ),
    (
        "an input for another family's stage is dropped",
        || Viewer {
            stage: image(),
            ..Viewer::default()
        },
        || ViewerIn::Stage(StageIn::Pdf(PdfIn::NextPage)),
        0,
        || Viewer {
            stage: image(),
            ..Viewer::default()
        },
        Vec::new,
    ),
    (
        "a quick look is promoted to a window",
        || Viewer::launched(Presentation::Peek),
        || ViewerIn::Presentation(PresentationIn::ToWindow),
        0,
        || Viewer::launched(Presentation::Window),
        || {
            vec![ViewerOut::Presentation(PresentationOut::Become(
                Presentation::Window,
            ))]
        },
    ),
    (
        "a probe that names a stage installs it and starts the peek and the open",
        || Viewer {
            load: Load::Probing { ticket: Ticket(1) },
            ..Viewer::default()
        },
        || {
            ViewerIn::Load(LoadIn::Probed {
                ticket: Ticket(1),
                flow: LoadFlow::PeekThenOpen,
                stage: StageFamily::Raster,
            })
        },
        0,
        || Viewer {
            load: Load::Peeking {
                ticket: Ticket(1),
                frame: PeekFrame::Pending,
            },
            stage: image(),
            ..Viewer::default()
        },
        || {
            vec![
                ViewerOut::Load(LoadOut::UseStage(StageFamily::Raster)),
                ViewerOut::Load(LoadOut::Peek(Ticket(1))),
                ViewerOut::Load(LoadOut::Open(Ticket(1))),
            ]
        },
    ),
    (
        "a probe result for a file already left installs nothing",
        || Viewer {
            load: Load::Probing { ticket: Ticket(2) },
            ..Viewer::default()
        },
        || {
            ViewerIn::Load(LoadIn::Probed {
                ticket: Ticket(1),
                flow: LoadFlow::PeekThenOpen,
                stage: StageFamily::Raster,
            })
        },
        0,
        || Viewer {
            load: Load::Probing { ticket: Ticket(2) },
            ..Viewer::default()
        },
        Vec::new,
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    let params = params();
    for (name, from, input, at, state, outs) in CASES {
        let (next, out) = from().step(input(), Stamp(*at), &(), &params);
        assert_eq!(next, state(), "{name}: state");
        assert_eq!(out, outs(), "{name}: outputs");
    }
}
