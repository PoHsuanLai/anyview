//! Reloading a file that changed, dropping files on the window, and the keys that scroll a text.

use super::support::*;
use crate::command::{Command, StageCommand};
use crate::load::{Load, LoadFlow, LoadIn, LoadOut, Ticket};
use crate::navigate::{Navigate, NavigateIn};
use crate::stage::{
    Animation, LineTotal, PageLines, RasterIn, RasterStage, Stage, StageFamily, StageIn,
    TextExtent, TextIn, TextParams, TextPlace, TextStage, TextStep, TextView, Wrap,
};
use crate::viewer::{Viewer, ViewerIn, ViewerOut, ViewerParams};
use anyview_core::{
    DocPoint, DocUnit, FilePath, LineIndex, NonEmpty, Permille, QuarterTurn, Sequence,
    SequenceOrigin, Zoom,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

fn zoomed_image() -> Stage {
    Stage::Raster(RasterStage::Zoomed {
        turn: QuarterTurn::Quarter,
        zoom: Zoom::Scale(Permille(2000)),
        centre: DocPoint {
            x: DocUnit(640),
            y: DocUnit(128),
        },
        anim: Animation::Still,
    })
}

fn probed(ticket: u64, stage: StageFamily) -> ViewerIn {
    ViewerIn::Load(LoadIn::Probed {
        ticket: Ticket(ticket),
        flow: LoadFlow::PeekThenOpen,
        stage,
    })
}

#[test]
fn a_reload_probes_the_file_again_and_leaves_the_stage_and_its_place_alone() {
    let params = params();
    let showing = Viewer {
        stage: zoomed_image(),
        load: Load::Ready { ticket: Ticket(4) },
        ..Viewer::default()
    };
    let (reloading, outs) =
        showing
            .clone()
            .step(ViewerIn::Reload(path("/a.png")), Stamp(0), &(), &params);
    assert_eq!(
        outs,
        vec![ViewerOut::Reload {
            ticket: Ticket(5),
            path: path("/a.png")
        }]
    );
    assert_eq!(reloading.stage, showing.stage, "the stage is not dropped");
    assert_eq!(reloading.load, Load::Probing { ticket: Ticket(5) });
    // The probe names the same family: the stage, with its zoom and centre, is the one that goes on.
    let (opening, _) = reloading.step(probed(5, StageFamily::Raster), Stamp(1), &(), &params);
    assert_eq!(opening.stage, showing.stage);
}

#[test]
fn a_reload_of_a_file_that_became_another_kind_gets_a_new_stage() {
    let params = params();
    let showing = Viewer {
        stage: zoomed_image(),
        load: Load::Ready { ticket: Ticket(4) },
        ..Viewer::default()
    };
    let (reloading, _) = showing.step(ViewerIn::Reload(path("/a.png")), Stamp(0), &(), &params);
    let (opening, _) = reloading.step(probed(5, StageFamily::Text), Stamp(1), &(), &params);
    assert_eq!(opening.stage.family(), StageFamily::Text);
}

#[test]
fn a_reload_abandons_a_load_still_in_flight_and_a_late_result_of_it_is_ignored() {
    let params = params();
    let (viewer, _) =
        Viewer::default().step(ViewerIn::Open(path("/a.png")), Stamp(0), &(), &params);
    let (viewer, outs) = viewer.step(ViewerIn::Reload(path("/a.png")), Stamp(1), &(), &params);
    assert_eq!(
        outs,
        vec![
            ViewerOut::Load(LoadOut::Cancel(Ticket(1))),
            ViewerOut::Reload {
                ticket: Ticket(2),
                path: path("/a.png")
            },
        ]
    );
    let (after, outs) = viewer
        .clone()
        .step(probed(1, StageFamily::Raster), Stamp(2), &(), &params);
    assert_eq!((after, outs), (viewer, vec![]));
}

fn dropped(paths: &[&str]) -> ViewerIn {
    ViewerIn::Dropped(paths.iter().map(|p| path(p)).collect())
}

#[test]
fn a_dropped_file_opens_and_its_folder_is_asked_for_as_the_list_to_walk() {
    let params = params();
    let files = NonEmpty::from_vec(vec![path("/old/1.png"), path("/old/2.png")]).unwrap();
    let sequence =
        Sequence::starting_at(files, &path("/old/1.png"), SequenceOrigin::Selection).unwrap();
    let (viewer, _) = Viewer {
        stage: zoomed_image(),
        ..Viewer::default()
    }
    .step(
        ViewerIn::Navigate(NavigateIn::Start(sequence)),
        Stamp(0),
        &(),
        &params,
    );
    let (viewer, outs) = viewer.step(dropped(&["/new/x.png"]), Stamp(1), &(), &params);
    assert_eq!(
        outs,
        vec![
            ViewerOut::Probe {
                ticket: Ticket(1),
                path: path("/new/x.png")
            },
            ViewerOut::ListFolder(path("/new/x.png")),
        ]
    );
    assert_eq!(viewer.stage, Stage::NoStage, "a dropped file opens afresh");
    assert_eq!(
        viewer.navigate,
        Navigate::Idle,
        "the old folder's arrows go nowhere until the new list arrives"
    );
}

#[test]
fn several_dropped_files_are_the_list_and_the_first_opens() {
    let params = params();
    let (viewer, outs) = Viewer::default().step(
        dropped(&["/a/1.png", "/b/2.png", "/a/3.png"]),
        Stamp(0),
        &(),
        &params,
    );
    let Navigate::Walking { sequence } = &viewer.navigate else {
        panic!("a walk starts");
    };
    assert_eq!(sequence.entries().count().get(), 3);
    assert_eq!(sequence.current(), &path("/a/1.png"));
    assert_eq!(
        outs,
        vec![
            ViewerOut::Probe {
                ticket: Ticket(1),
                path: path("/a/1.png")
            },
            ViewerOut::Preload(anyview_core::Neighbours {
                previous: None,
                next: Some(path("/b/2.png")),
            }),
        ],
        "no folder is listed: the files dropped are the list"
    );
}

#[test]
fn dropping_nothing_changes_nothing() {
    let params = params();
    let (viewer, outs) =
        Viewer::default().step(ViewerIn::Dropped(Vec::new()), Stamp(0), &(), &params);
    assert_eq!((viewer, outs), (Viewer::default(), vec![]));
}

fn reading_at(line: u32) -> Stage {
    Stage::Text(TextStage::Reading {
        place: TextPlace {
            line: LineIndex(line),
            wrap: Wrap::On,
            view: TextView::Source,
        },
    })
}

fn long_text() -> ViewerParams {
    let mut params = params();
    params.stage.text = TextParams {
        extent: TextExtent {
            lines: LineTotal(500),
            page: PageLines(30),
        },
        ..TextParams::default()
    };
    params
}

fn pressed(viewer: Viewer, keys: &[ShortcutKey]) -> Viewer {
    viewer
        .step(
            ViewerIn::Key(Shortcut(keys.to_vec())),
            Stamp(0),
            &(),
            &long_text(),
        )
        .0
}

fn line_of(viewer: &Viewer) -> Option<u32> {
    match &viewer.stage {
        Stage::Text(TextStage::Reading { place } | TextStage::Finding { place, .. }) => {
            Some(place.line.0)
        }
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

#[test]
fn the_keys_scroll_a_text_by_line_and_page_and_to_its_ends() {
    use ShortcutKey::{Down, End, Home, PageDown, PageUp, Up};
    // name, line before, key, line after
    const CASES: &[(&str, u32, ShortcutKey, u32)] = &[
        ("down", 10, Down, 11),
        ("up", 10, Up, 9),
        ("page down", 10, PageDown, 40),
        ("page up", 100, PageUp, 70),
        ("home", 250, Home, 0),
        ("end", 10, End, 470),
    ];
    for (name, before, key, after) in CASES {
        let viewer = pressed(
            Viewer {
                stage: reading_at(*before),
                ..Viewer::default()
            },
            &[*key],
        );
        assert_eq!(line_of(&viewer), Some(*after), "{name}");
    }
}

#[test]
fn home_and_end_still_walk_the_folder_for_a_file_that_does_not_scroll() {
    let files = NonEmpty::from_vec(vec![path("/a.png"), path("/b.png"), path("/c.png")]).unwrap();
    let sequence =
        Sequence::starting_at(files, &path("/b.png"), SequenceOrigin::Selection).unwrap();
    let viewer = Viewer {
        stage: zoomed_image(),
        navigate: Navigate::Walking { sequence },
        ..Viewer::default()
    };
    let (_, outs) = viewer.step(
        ViewerIn::Key(Shortcut(vec![ShortcutKey::End])),
        Stamp(0),
        &(),
        &params(),
    );
    assert!(
        outs.contains(&ViewerOut::Probe {
            ticket: Ticket(1),
            path: path("/c.png")
        }),
        "End on an image goes to the last file: {outs:?}"
    );
}

#[test]
fn a_command_to_step_the_text_is_the_same_as_its_key() {
    let viewer = Viewer {
        stage: reading_at(10),
        ..Viewer::default()
    };
    let (viewer, _) = viewer.step(
        ViewerIn::Run(Command::Stage(StageCommand::LineDown)),
        Stamp(0),
        &(),
        &long_text(),
    );
    assert_eq!(line_of(&viewer), Some(11));
    let input = StageIn::Text(TextIn::Step(TextStep::LineDown));
    let (again, _) = viewer.step(ViewerIn::Stage(input), Stamp(1), &(), &long_text());
    assert_eq!(line_of(&again), Some(12));
}

#[test]
fn a_restore_input_is_taken_by_a_fresh_raster_stage() {
    let (viewer, _) = Viewer {
        stage: Stage::Raster(RasterStage::default()),
        ..Viewer::default()
    }
    .step(
        ViewerIn::Stage(StageIn::Raster(RasterIn::Restore {
            zoom: Zoom::Actual,
            centre: DocPoint::default(),
        })),
        Stamp(0),
        &(),
        &params(),
    );
    assert_eq!(
        viewer.stage.resume(),
        anyview_core::Resume::Raster {
            zoom: Zoom::Actual,
            centre: DocPoint::default()
        }
    );
}
