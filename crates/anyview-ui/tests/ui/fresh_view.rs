//! A different file opens as the first one did: its zoom, scroll, find, pan position and window
//! size are its own, whatever the last file was left at, for every family and at both scales. The
//! pointer tool and the side panel are the person's, and go with them from file to file. Resume is
//! the one memory of a file, and it belongs to the file it was kept for.

use crate::support;

use anyview_core::{DocPoint, DocUnit, Permille, Resume, Zoom};
use anyview_core::{PixelLen, PixelSize};
use anyview_ui::{HostRequest, NaturalSize, SizeBasis};
use ds::file_drop::drag::{FileDragInput, Offer};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use support::{Memory, PanelStart, Requests, Wiring, settle, wired};

const SCALES: [u16; 2] = [100, 200];

#[derive(Clone, Copy, Debug)]
enum Arrival {
    Arrow,
    Drop,
}

fn at(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}

/// The Info action's chord (Command and Option with I).
fn info(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('i')));
    settle(harness);
}

fn press(harness: &mut Harness, key: ShortcutKey) {
    harness.send(Input::key(key));
    settle(harness);
}

fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

/// Everything on screen that is the view's state, as text.
fn view_of(harness: &Harness) -> String {
    format!(
        "{} | rows {:?} | zoom {:?} | place {:?} | line {:?} | palette {} | page {:?}",
        capsule(harness),
        harness
            .text_of(".viewer-data")
            .map(|text| text.chars().take(120).collect::<String>()),
        harness.attr(".viewer-raster", "data-zoom"),
        harness.attr(".viewer-raster-picture", "style"),
        harness.text_of(".viewer-lineno"),
        harness.count(".ds-palette"),
        harness.attr(".viewer-pdf", "style"),
    )
}

/// A scratch folder of two copies of one fixture.
fn pair(crate_dir: &str, fixture: &str, ext: &str) -> (tempfile::TempDir, Vec<PathBuf>) {
    let (dir, mut paths) = support::folder(&[
        (crate_dir, fixture, &format!("a.{ext}")),
        (crate_dir, fixture, &format!("b.{ext}")),
    ]);
    paths.truncate(2);
    (dir, paths)
}

fn numbered(name: &str) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let paths = ["a", "b"]
        .iter()
        .map(|stem| support::text_file(dir.path(), &format!("{stem}.{name}"), "line", 300))
        .collect();
    (dir, paths)
}

fn long_table() -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let body: String = std::iter::once("id,name\n".to_owned())
        .chain((1..=400).map(|n| format!("{n},row{n}\n")))
        .collect();
    let paths = ["a", "b"]
        .iter()
        .map(|stem| {
            let path = dir.path().join(format!("{stem}.csv"));
            std::fs::write(&path, &body).unwrap();
            std::fs::canonicalize(path).unwrap()
        })
        .collect();
    (dir, paths)
}

fn viewport(scale: u16) -> Wiring {
    Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..Wiring::default()
    }
}

fn next(harness: &mut Harness, how: Arrival, to: &std::path::Path) {
    match how {
        Arrival::Arrow => press(harness, ShortcutKey::Right),
        Arrival::Drop => {
            for step in [
                FileDragInput::Entered {
                    point: Some(at(450.0, 300.0)),
                },
                FileDragInput::Offered(Offer::Files(vec![to.to_path_buf()])),
                FileDragInput::Moved {
                    point: at(450.0, 300.0),
                },
                FileDragInput::Dropped,
            ] {
                harness.send(Input::FileDrag(step));
            }
        }
    }
    settle(harness);
    settle(harness);
}

/// Change what can be changed in a file of this family.
type Change = fn(&mut Harness);

/// A family's name, its two files, and the change.
type Family = (&'static str, (tempfile::TempDir, Vec<PathBuf>), Change);

fn zoom_pan_and_hand(harness: &mut Harness) {
    for _ in 0..4 {
        press(harness, ShortcutKey::Char('+'));
    }
    press(harness, ShortcutKey::Char('h'));
    harness.send(Input::wheel(at(450.0, 300.0), Px(20.0), Px(20.0)));
    settle(harness);
}

fn scroll_and_find(harness: &mut Harness) {
    harness.send(Input::wheel(at(450.0, 300.0), Px(0.0), Px(-180.0)));
    settle(harness);
    press(harness, ShortcutKey::PageDown);
}

fn page_and_zoom(harness: &mut Harness) {
    press(harness, ShortcutKey::PageDown);
    press(harness, ShortcutKey::Char('+'));
}

fn rows(harness: &mut Harness) {
    harness.send(Input::wheel(at(450.0, 300.0), Px(0.0), Px(-600.0)));
    settle(harness);
}

#[test]
fn a_different_file_starts_with_a_view_of_its_own_in_every_family() {
    // family, files, how the first is changed
    let families: Vec<Family> = vec![
        (
            "picture",
            pair("anyview-image", "quadrants.png", "png"),
            zoom_pan_and_hand,
        ),
        ("text", numbered("txt"), scroll_and_find),
        (
            "pdf",
            pair("anyview-ui", "formats/multi.pdf", "pdf"),
            page_and_zoom,
        ),
        ("table", long_table(), rows),
    ];
    for scale in SCALES {
        for how in [Arrival::Arrow, Arrival::Drop] {
            for (name, (_dir, paths), change) in &families {
                let (mut harness, _, _) = wired(paths, 0, Appearance::default(), viewport(scale));
                settle(&mut harness);
                harness.send(Input::pointer_move(at(450.0, 300.0)));
                settle(&mut harness);
                let fresh = view_of(&harness);
                change(&mut harness);
                assert_ne!(
                    view_of(&harness),
                    fresh,
                    "{scale} {name}: the change changed nothing"
                );
                next(&mut harness, how, &paths[1]);
                assert_eq!(
                    view_of(&harness),
                    fresh,
                    "{scale} {how:?} {name}: the second file is not at the first's state"
                );
            }
        }
    }
}

fn quadrants() -> SizeBasis {
    SizeBasis::Natural(NaturalSize::Pixels(PixelSize {
        width: PixelLen(48),
        height: PixelLen(32),
    }))
}

#[test]
fn each_different_file_sizes_the_window_once_and_a_resume_belongs_to_its_own_file() {
    for scale in SCALES {
        let (_dir, paths) = pair("anyview-image", "quadrants.png", "png");
        let zoom = Resume::Raster {
            zoom: Zoom::Scale(Permille(2000)),
            centre: DocPoint {
                x: DocUnit(10),
                y: DocUnit(10),
            },
        };
        let memory = Memory::with(&paths[0], zoom);
        let wiring = Wiring {
            memory: Some(memory),
            ..viewport(scale)
        };
        let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), wiring);
        settle(&mut harness);
        assert_eq!(
            harness.attr(".viewer-raster", "data-zoom").as_deref(),
            Some("zoomed"),
            "{scale}: the same file resumes where it was"
        );
        next(&mut harness, Arrival::Arrow, &paths[1]);
        assert_eq!(
            harness.attr(".viewer-raster", "data-zoom").as_deref(),
            Some("fit"),
            "{scale}: another file does not"
        );
        assert_eq!(
            sizes_of(&requests),
            vec![quadrants(), quadrants()],
            "{scale}: one size for each file, each its picture's own"
        );
        press(&mut harness, ShortcutKey::Left);
        settle(&mut harness);
        assert_eq!(
            harness.attr(".viewer-raster", "data-zoom").as_deref(),
            Some("zoomed"),
            "{scale}: coming back to the first file restores its resume"
        );
        assert_eq!(
            sizes_of(&requests).len(),
            3,
            "{scale}: and sizes the window to it again"
        );
    }
}

#[test]
fn the_pointer_tool_goes_with_the_person_to_the_next_file_and_the_pan_does_not() {
    for scale in SCALES {
        let (_dir, paths) = pair("anyview-image", "quadrants.png", "png");
        let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), viewport(scale));
        settle(&mut harness);
        harness.send(Input::pointer_move(at(450.0, 300.0)));
        settle(&mut harness);
        let fresh_place = harness.attr(".viewer-raster-picture", "style");
        assert_eq!(
            harness.attr(".viewer-raster", "data-pan").as_deref(),
            Some("on"),
            "{scale}: Pan to begin with"
        );
        zoom_pan_and_hand(&mut harness);
        assert_eq!(
            harness.attr(".viewer-raster", "data-pan").as_deref(),
            Some("off"),
            "{scale}: H chose Select"
        );
        next(&mut harness, Arrival::Arrow, &paths[1]);
        assert_eq!(
            harness.attr(".viewer-raster", "data-pan").as_deref(),
            Some("off"),
            "{scale}: the next file is still in Select"
        );
        assert_eq!(
            harness.attr(".viewer-raster-picture", "style"),
            fresh_place,
            "{scale}: but its picture is where a fresh one sits"
        );
    }
}

/// How the second file of a pair is spoiled.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum Damage {
    /// The file cannot be read at all, so its type cannot be read either.
    Unreadable,
    /// The header of a picture and nothing after it, so the type is known and the open fails.
    Truncated,
}

#[cfg(unix)]
impl Damage {
    fn apply(self, path: &std::path::Path) {
        match self {
            Damage::Unreadable => {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
            }
            Damage::Truncated => std::fs::write(path, b"\x89PNG\r\n\x1a\n\0\0").unwrap(),
        }
    }
}

#[cfg(unix)]
#[test]
fn a_file_that_fails_to_load_sizes_the_window_as_the_default_not_as_the_last_file() {
    for scale in SCALES {
        // A file whose probe fails, and one whose open does.
        for (name, damage) in [
            ("unreadable", Damage::Unreadable),
            ("cut short", Damage::Truncated),
        ] {
            let (_dir, paths) = pair("anyview-image", "quadrants.png", "png");
            // Spoiled before the window opens, or the neighbour would be read ahead whole.
            damage.apply(&paths[1]);
            let (mut harness, requests, _) =
                wired(&paths, 0, Appearance::default(), viewport(scale));
            settle(&mut harness);
            next(&mut harness, Arrival::Arrow, &paths[1]);
            assert_eq!(
                sizes_of(&requests),
                vec![quadrants(), SizeBasis::Default],
                "{scale} {name}: the second file asks for the default window"
            );
        }
    }
}

#[test]
fn the_side_panel_stays_open_from_file_to_file_on_the_same_tab_or_the_first() {
    for scale in SCALES {
        let (_dir, mut files) = support::folder(&[
            ("anyview-ui", "formats/multi.pdf", "a.pdf"),
            ("anyview-ui", "formats/multi.pdf", "b.pdf"),
            ("anyview-image", "quadrants.png", "c.png"),
        ]);
        files.truncate(3);
        let wiring = Wiring {
            panel: PanelStart::AsOpened,
            ..viewport(scale)
        };
        let (mut harness, _, _) = wired(&files, 0, Appearance::default(), wiring);
        settle(&mut harness);
        // The panel opened on the pages. (A click on a tab would leave the keyboard in the tab
        // control, whose arrows move between tabs, so the arrow below would not walk the folder.)
        let tabs = |harness: &Harness| {
            (
                harness.attr(".ds-split-pane", "data-shown"),
                harness.text_of(".viewer-panel .ds-segmented"),
                harness.text_of(".viewer-panel [data-selected=selected]"),
            )
        };
        let on_pdf = tabs(&harness);
        assert_eq!(on_pdf.0.as_deref(), Some("visible"), "{scale}: {on_pdf:?}");
        assert!(
            on_pdf.2.as_deref().is_some_and(|tab| !tab.is_empty()),
            "{scale}: a tab is chosen: {on_pdf:?}"
        );
        next(&mut harness, Arrival::Arrow, &files[1]);
        assert_eq!(
            tabs(&harness),
            on_pdf,
            "{scale}: the next PDF keeps the panel and its tab"
        );
        next(&mut harness, Arrival::Arrow, &files[2]);
        let on_picture = tabs(&harness);
        assert_eq!(
            on_picture.0.as_deref(),
            Some("visible"),
            "{scale}: a picture keeps the panel open too: {on_picture:?}"
        );
        assert!(
            harness
                .text_of(".viewer-panel")
                .unwrap_or_default()
                .contains("48"),
            "{scale}: on its Info tab, the picture's facts"
        );
    }
}

fn sizes_of(requests: &Requests) -> Vec<SizeBasis> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::SizeWindow(size) = request {
                Some(*size)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn a_pdf_opens_its_panel_on_the_pages_and_a_closed_panel_stays_closed_for_the_next() {
    for scale in SCALES {
        let (_dir, files) = pair("anyview-ui", "formats/multi.pdf", "pdf");
        let wiring = Wiring {
            panel: PanelStart::AsOpened,
            ..viewport(scale)
        };
        let (mut harness, _, _) = wired(&files, 0, Appearance::default(), wiring);
        settle(&mut harness);
        assert_eq!(
            harness.attr(".ds-split-pane", "data-shown").as_deref(),
            Some("visible"),
            "{scale}: the pages are beside the first page"
        );
        assert!(
            harness.count(".viewer-panel .ds-segmented") > 0,
            "{scale}: the pages tab is one of several"
        );
        let on_pages = harness.text_of(".viewer-panel [data-selected=selected]");
        assert!(
            on_pages.is_some_and(|label| !label.is_empty()),
            "{scale}: a tab is chosen"
        );
        press(&mut harness, ShortcutKey::Escape);
        assert_eq!(
            harness.attr(".ds-split-pane", "data-shown"),
            None,
            "{scale}: Esc puts the panel away"
        );
        next(&mut harness, Arrival::Arrow, &files[1]);
        assert_eq!(
            harness.attr(".ds-split-pane", "data-shown"),
            None,
            "{scale}: the next PDF does not bring back a panel the person closed"
        );
    }
}

#[test]
fn a_file_that_is_only_its_card_has_no_panel_to_open() {
    for scale in SCALES {
        let dir = tempfile::tempdir().unwrap();
        let blob = dir.path().join("blob.xyz");
        std::fs::write(&blob, [0u8, 159, 146, 150, 1, 2, 3, 0, 0, 255]).unwrap();
        let blob = std::fs::canonicalize(blob).unwrap();
        let wiring = Wiring {
            panel: PanelStart::AsOpened,
            ..viewport(scale)
        };
        let (mut harness, _, _) = wired(&[blob], 0, Appearance::default(), wiring);
        settle(&mut harness);
        info(&mut harness);
        assert_eq!(
            harness.attr(".ds-split-pane", "data-shown"),
            None,
            "{scale}: the stage is the card, so Info opens nothing beside it"
        );
        assert!(
            harness.count(".viewer-peek") > 0,
            "{scale}: the card is the stage"
        );
    }
}
