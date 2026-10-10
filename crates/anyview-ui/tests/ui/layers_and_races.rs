//! Layers that stack (panel, palette, menu, sheet) and commands that arrive while a file is still
//! loading.

use crate::support;

use anyview_ui::{HostRequest, Presentation, Work, WorkKind, Workers};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use support::{Requests, Wiring, press, settle, text_file, wired};

const SCALES: [u16; 2] = [100, 200];

fn two(scale: u16, wiring: Wiring) -> (tempfile::TempDir, Vec<PathBuf>, Harness, Requests) {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<PathBuf> = (1..=3)
        .map(|n| text_file(dir.path(), &format!("f{n}.txt"), &format!("w{n}"), 5))
        .collect();
    let wiring = Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent: scale,
        }),
        ..wiring
    };
    let (mut harness, requests, _) = wired(&files, 0, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, files, harness, requests)
}

/// What the panel's pane says of itself.
fn panel(harness: &Harness) -> String {
    harness
        .attr(".ds-split-pane", "data-shown")
        // A folded panel is not in the split view at all.
        .unwrap_or_else(|| "hidden".to_owned())
}

fn closes(requests: &Requests) -> usize {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, HostRequest::CloseWindow))
        .count()
}

/// The Info action's chord (Command and Option with I).
fn info(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('i')));
    settle(harness);
}

fn chord(harness: &mut Harness, key: char) {
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char(key)));
    settle(harness);
}

#[test]
fn escape_closes_the_palette_before_the_panel_and_the_panel_before_the_window() {
    for scale in SCALES {
        let wiring = Wiring {
            presentation: Presentation::Peek,
            ..Wiring::default()
        };
        let (_dir, _, mut harness, requests) = two(scale, wiring);
        info(&mut harness);
        assert_eq!(panel(&harness), "visible", "{scale}: the panel is up");
        chord(&mut harness, 'k');
        assert_eq!(harness.count(".ds-palette"), 1, "{scale}");
        press(&mut harness, ShortcutKey::Escape);
        assert_eq!(
            harness.count(".ds-palette"),
            0,
            "{scale}: the palette goes first"
        );
        assert_eq!(panel(&harness), "visible", "{scale}: the panel stays");
        assert_eq!(closes(&requests), 0, "{scale}");
        press(&mut harness, ShortcutKey::Escape);
        assert_eq!(panel(&harness), "hidden", "{scale}: then the panel");
        assert_eq!(closes(&requests), 0, "{scale}");
        press(&mut harness, ShortcutKey::Escape);
        assert_eq!(closes(&requests), 1, "{scale}: then the quick look");
    }
}

#[test]
fn escape_closes_the_menu_and_leaves_the_panel() {
    for scale in SCALES {
        let (_dir, _, mut harness, _) = two(scale, Wiring::default());
        info(&mut harness);
        harness.send(Input::key(ShortcutKey::ContextMenu));
        settle(&mut harness);
        assert_eq!(harness.count(".ds-menu"), 1, "{scale}: the menu is up");
        press(&mut harness, ShortcutKey::Escape);
        assert_eq!(harness.count(".ds-menu"), 0, "{scale}");
        assert_eq!(panel(&harness), "visible", "{scale}: the panel stays");
    }
}

/// Workers that run everything until told to hold the opening of files, then keep those jobs
/// until they are let go.
#[derive(Default)]
struct LoadsOnHold {
    holding: Mutex<bool>,
    held: Mutex<Vec<Work>>,
}

impl LoadsOnHold {
    fn hold(&self) {
        *self.holding.lock().unwrap() = true;
    }

    fn release(&self) {
        *self.holding.lock().unwrap() = false;
        let held = std::mem::take(&mut *self.held.lock().unwrap());
        for work in held {
            work.run();
        }
    }
}

impl Workers for LoadsOnHold {
    fn submit(&self, work: Work) {
        let hold = *self.holding.lock().unwrap()
            && matches!(
                work.kind(),
                WorkKind::Probe | WorkKind::Open | WorkKind::Peek | WorkKind::Preload
            );
        if hold {
            self.held.lock().unwrap().push(work);
        } else {
            work.run();
        }
    }
}

fn loading_the_second(scale: u16) -> (Arc<LoadsOnHold>, Harness, Requests, tempfile::TempDir) {
    let hold = Arc::new(LoadsOnHold::default());
    let wiring = Wiring {
        workers: Some(hold.clone()),
        ..Wiring::default()
    };
    let (dir, _, mut harness, requests) = two(scale, wiring);
    hold.hold();
    // f2 was opened ahead; f3 is not, so the second arrow has to probe it.
    press(&mut harness, ShortcutKey::Right);
    press(&mut harness, ShortcutKey::Right);
    (hold, harness, requests, dir)
}

/// The name of the file the host was last told is open.
fn host_shows(requests: &Requests) -> Option<String> {
    requests.lock().unwrap().iter().rev().find_map(|request| {
        let HostRequest::Opened(probed) = request else {
            return None;
        };
        probed
            .source
            .path()
            .file_name()
            .map(|name| name.as_str().to_owned())
    })
}

#[test]
fn moving_to_the_trash_while_the_next_file_loads_trashes_the_file_the_sheet_names() {
    for scale in SCALES {
        let (hold, mut harness, requests, _dir) = loading_the_second(scale);
        harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Backspace));
        settle(&mut harness);
        let named = harness.text_of(".ds-alert").unwrap_or_default();
        let Some(button) = harness.centre(".ds-alert-footer .ds-alert-slot:last-child .ds-button")
        else {
            hold.release();
            continue; // no sheet while loading: nothing can go wrong
        };
        harness.send(Input::click(button));
        settle(&mut harness);
        let asked = requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| matches!(request, HostRequest::Trash));
        let host = host_shows(&requests).unwrap_or_default();
        hold.release();
        assert!(
            !asked || named.contains(&host),
            "{scale}: the sheet says {named:?}; the host, which trashes the file it was last told is open, has {host:?}"
        );
    }
}
