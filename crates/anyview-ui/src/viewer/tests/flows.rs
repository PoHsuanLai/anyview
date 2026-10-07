use super::support::*;
use crate::chrome::*;
use crate::load::*;
use crate::navigate::*;
use crate::palette::*;
use crate::panel::*;
use crate::presentation::*;
use crate::stage::*;
use crate::testing::settle;
use crate::viewer::*;
use anyview_core::*;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey, Shown};

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

fn key(keys: &[ShortcutKey]) -> ViewerIn {
    ViewerIn::Key(Shortcut(keys.to_vec()))
}

#[test]
fn opening_a_file_probes_it_and_drops_the_stage_of_the_last_one() {
    let params = params();
    let viewer = Viewer {
        stage: image(),
        ..Viewer::default()
    };
    let (viewer, outs) = viewer.step(ViewerIn::Open(path("/a.png")), Stamp(0), &(), &params);
    assert_eq!(
        outs,
        vec![ViewerOut::Probe {
            ticket: Ticket(1),
            path: path("/a.png")
        }]
    );
    assert_eq!(viewer.stage, Stage::NoStage);
    // A second open abandons the first: the old ticket is cancelled and a new one probed.
    let (viewer, outs) = viewer.step(ViewerIn::Open(path("/b.png")), Stamp(1), &(), &params);
    assert_eq!(
        outs,
        vec![
            ViewerOut::Load(LoadOut::Cancel(Ticket(1))),
            ViewerOut::Probe {
                ticket: Ticket(2),
                path: path("/b.png")
            },
        ]
    );
    // The first file's probe answers late and changes nothing.
    let late = ViewerIn::Load(LoadIn::Probed {
        ticket: Ticket(1),
        flow: LoadFlow::OpenOnly,
        stage: StageFamily::Media,
    });
    let (after, outs) = viewer.clone().step(late, Stamp(2), &(), &params);
    assert_eq!((after, outs), (viewer, vec![]));
}

#[test]
fn the_arrow_keys_walk_the_sequence_and_preload_around_the_new_file() {
    let params = params();
    let files = NonEmpty::from_vec(vec![path("/a.png"), path("/b.png"), path("/c.png")]).unwrap();
    let sequence =
        Sequence::starting_at(files, &path("/a.png"), SequenceOrigin::Selection).unwrap();
    let (viewer, _) = Viewer::default().step(
        ViewerIn::Navigate(NavigateIn::Start(sequence)),
        Stamp(0),
        &(),
        &params,
    );
    let (viewer, outs) = viewer.step(key(&[ShortcutKey::Right]), Stamp(1), &(), &params);
    assert_eq!(
        outs,
        vec![
            ViewerOut::Probe {
                ticket: Ticket(1),
                path: path("/b.png")
            },
            ViewerOut::Preload(anyview_core::Neighbours {
                previous: Some(path("/a.png")),
                next: Some(path("/c.png")),
            }),
        ]
    );
    assert_eq!(viewer.load, Load::Probing { ticket: Ticket(1) });
}

/// The viewer after `keys`, and what it asked for.
fn pressed(viewer: Viewer, keys: &[ShortcutKey], at: u64) -> (Viewer, Vec<ViewerOut>) {
    viewer.step(key(keys), Stamp(at), &(), &params())
}

#[test]
fn keys_open_and_close_the_regions_in_the_order_they_are_open() {
    use ShortcutKey::{Char, Escape, Super};
    // Command K opens the palette; Esc closes it, and only it.
    let (viewer, outs) = pressed(Viewer::default(), &[Super, Char('k')], 0);
    assert_eq!(outs, vec![ViewerOut::Palette(PaletteOut::Opened), FADE_IN]);
    assert_eq!(viewer.palette, palette_on(0));
    let (viewer, _) = pressed(viewer, &[Escape], 1);
    assert_eq!(viewer.palette, Palette::Closed);
    // Command I shows the info tab, and Esc closes the panel next.
    let (viewer, outs) = pressed(viewer, &[Super, Char('i')], 2);
    assert_eq!(outs, vec![ViewerOut::Panel(PanelOut::Show(PanelTab::Info))]);
    let (viewer, outs) = pressed(viewer, &[Escape], 3);
    assert_eq!(outs, vec![ViewerOut::Panel(PanelOut::Hide)]);
    // With nothing open Esc does nothing in a window, and command W closes it.
    let (viewer, outs) = pressed(viewer, &[Escape], 4);
    assert_eq!(outs, vec![]);
    let (_, outs) = pressed(viewer, &[Super, Char('w')], 5);
    assert_eq!(outs, vec![ViewerOut::CloseWindow]);
}

#[test]
fn escape_closes_a_quick_look_and_leaves_a_window() {
    let params = params();
    let esc = || key(&[ShortcutKey::Escape]);
    let (_, peek) = Viewer::launched(Presentation::Peek).step(esc(), Stamp(0), &(), &params);
    assert_eq!(peek, vec![ViewerOut::CloseWindow]);
    for presentation in [
        Presentation::Window,
        Presentation::Mini,
        Presentation::Background,
    ] {
        let (_, outs) = Viewer::launched(presentation).step(esc(), Stamp(0), &(), &params);
        assert_eq!(outs, vec![], "{presentation:?}");
    }
}

#[test]
fn a_viewer_at_rest_runs_no_timer_and_the_chrome_sets_the_only_wake() {
    let params = params();
    assert_eq!(Viewer::default().wake(), None);
    let (viewer, _) = Viewer::default().step(
        ViewerIn::Chrome(ChromeIn::PointerMoved(Zone::Content)),
        Stamp(1000),
        &(),
        &params,
    );
    assert_eq!(viewer.wake(), Some(Stamp(1150)));
}

#[test]
fn the_chrome_hides_through_the_root_at_exactly_the_idle_deadline() {
    let params = params();
    let (viewer, _) = Viewer::default().step(
        ViewerIn::Chrome(ChromeIn::PointerMoved(Zone::Content)),
        Stamp(1000),
        &(),
        &params,
    );
    let (rest, log) = settle(viewer, &(), &params, 10);
    assert_eq!(rest, Viewer::default());
    let fade_out = ViewerOut::Chrome(ChromeOut::Fade {
        to: Shown::Hidden,
        over: QUICK,
    });
    assert_eq!(log, vec![(Stamp(3150), fade_out)]);
}

/// What the sheet asked of the window in `outs`, in order.
fn sheet_outs(outs: &[ViewerOut]) -> Vec<crate::sheet::SheetOut> {
    outs.iter()
        .filter_map(|out| {
            if let ViewerOut::Sheet(out) = out {
                Some(out.clone())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn an_install_button_opens_the_question_and_the_sheet_then_takes_the_keys() {
    use crate::command::Command;
    use crate::sheet::{HelperEnd, HelperPhase, Sheet, SheetIn, SheetOut};
    let params = params();
    let helper = Helper::HeicDecode;
    let viewer = Viewer {
        stage: image(),
        ..Viewer::default()
    };
    let (viewer, outs) = viewer.step(
        ViewerIn::Run(Command::Install(helper)),
        Stamp(0),
        &(),
        &params,
    );
    assert_eq!(
        viewer.sheet,
        Sheet::Helper {
            helper,
            phase: HelperPhase::Ask
        }
    );
    assert_eq!(sheet_outs(&outs), [SheetOut::Opened]);
    assert!(
        matches!(&viewer.chrome, Chrome::Pinned { by } if *by == PinReasons::of(PinReason::MenuOpen)),
        "the chrome is held while the sheet is up: {:?}",
        viewer.chrome
    );

    // Return installs, as the default button does.
    let (viewer, outs) = viewer.step(key(&[ShortcutKey::Enter]), Stamp(0), &(), &params);
    assert_eq!(sheet_outs(&outs), [SheetOut::Provide(helper)]);

    // The host's answer reopens the file, and the sheet is gone.
    let (viewer, outs) = viewer.step(
        ViewerIn::Sheet(SheetIn::HelperEnded(helper, HelperEnd::Installed)),
        Stamp(0),
        &(),
        &params,
    );
    assert_eq!(viewer.sheet, Sheet::Closed);
    assert_eq!(sheet_outs(&outs), [SheetOut::Reopen, SheetOut::Closed]);
}

#[test]
fn escape_is_not_now_and_asks_nothing() {
    use crate::command::Command;
    use crate::sheet::{Sheet, SheetOut};
    let params = params();
    let viewer = Viewer {
        stage: image(),
        ..Viewer::default()
    };
    let (viewer, _) = viewer.step(
        ViewerIn::Run(Command::Install(Helper::VideoPlayback)),
        Stamp(0),
        &(),
        &params,
    );
    let (viewer, outs) = viewer.step(key(&[ShortcutKey::Escape]), Stamp(0), &(), &params);
    assert_eq!(viewer.sheet, Sheet::Closed);
    assert_eq!(sheet_outs(&outs), [SheetOut::Closed]);
}
