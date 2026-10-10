//! The keyboard of a pane: the host owns every chord and the focus, the pane takes plain keys
//! while it is focused, and Esc undoes the innermost thing before it gives the keyboard back.

use crate::support::{Host, text_file, walking};
use anyview_pane::PaneRequest;
use dioxus::prelude::Key;
use ds::prelude::ShortcutKey;
use ds_harness::{Driver, Input, Query, Viewport};
use std::path::PathBuf;

fn in_a_corner() -> Viewport {
    Viewport {
        width: 480,
        height: 320,
        scale_percent: 100,
    }
}

/// Three text files, the first open.
fn three(dir: &tempfile::TempDir) -> Host {
    let paths: Vec<PathBuf> = ["a", "b", "c"]
        .iter()
        .map(|word| text_file(dir.path(), &format!("{word}.txt"), word, 40))
        .collect();
    walking(&paths, 0, in_a_corner())
}

fn press(host: &mut Host, key: ShortcutKey) {
    host.harness.send(Input::key(key));
    host.settle();
}

#[test]
fn the_arrow_keys_walk_the_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = three(&dir);
    assert_eq!(host.showing().as_deref(), Some("a.txt"));
    // key, the file shown after it
    for (key, want) in [
        (ShortcutKey::Right, "b.txt"),
        (ShortcutKey::Right, "c.txt"),
        (ShortcutKey::Left, "b.txt"),
    ] {
        press(&mut host, key);
        assert_eq!(host.showing().as_deref(), Some(want), "{key:?}");
    }
}

#[test]
fn the_hosts_chords_are_left_unconsumed_and_a_plain_key_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = three(&dir);
    host.heard.lock().unwrap().clear();
    // The palette, Info and Open: the viewer's own in a window, the host's in a pane.
    for (held, key) in [
        (ShortcutKey::Super, ShortcutKey::Char('k')),
        (ShortcutKey::Super, ShortcutKey::Char('i')),
        (ShortcutKey::Ctrl, ShortcutKey::Char('o')),
    ] {
        host.harness.send(Input::chord(&[held], key));
        host.settle();
    }
    {
        let heard = host.heard.lock().unwrap();
        assert!(!heard.is_empty(), "the keys reached the host");
        assert!(
            heard.iter().all(|key| !key.taken),
            "the pane took no chord: {heard:?}"
        );
    }
    assert_eq!(host.harness.count(".ds-palette"), 0);
    assert_eq!(host.harness.count(".viewer-panel"), 0);
    assert_eq!(host.shown(), ["a.txt"], "and opened nothing");

    host.heard.lock().unwrap().clear();
    press(&mut host, ShortcutKey::Right);
    assert!(
        host.heard
            .lock()
            .unwrap()
            .iter()
            .any(|key| key.key == Key::ArrowRight && key.taken),
        "an arrow is the pane's"
    );
}

#[test]
fn escape_undoes_what_is_open_and_then_gives_the_keyboard_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = three(&dir);
    press(&mut host, ShortcutKey::ContextMenu);
    assert_eq!(host.harness.count(".ds-menu"), 1, "the menu is open");

    press(&mut host, ShortcutKey::Escape);
    assert_eq!(host.harness.count(".ds-menu"), 0, "the first Esc closes it");
    assert!(!host.asked(&PaneRequest::Unfocus), "and keeps the keyboard");

    press(&mut host, ShortcutKey::Escape);
    assert!(host.asked(&PaneRequest::Unfocus), "the next gives it back");
    assert!(
        !host.asked(&PaneRequest::ClosePane),
        "and never closes the pane"
    );
    assert_eq!(host.showing().as_deref(), Some("a.txt"));
}

#[test]
fn a_pane_the_host_has_not_focused_handles_no_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = three(&dir);
    host.focus(false);
    host.heard.lock().unwrap().clear();
    press(&mut host, ShortcutKey::Right);
    press(&mut host, ShortcutKey::Escape);
    assert_eq!(host.showing().as_deref(), Some("a.txt"), "it did not walk");
    assert!(!host.asked(&PaneRequest::Unfocus));
    assert!(
        host.heard.lock().unwrap().iter().all(|key| !key.taken),
        "and took nothing from the host"
    );

    host.focus(true);
    press(&mut host, ShortcutKey::Right);
    assert_eq!(
        host.showing().as_deref(),
        Some("b.txt"),
        "given the keyboard back, it takes it"
    );
}

#[test]
fn losing_the_focus_to_the_host_gives_the_keyboard_back_once() {
    // what happens first, how many times the pane asked to give the keyboard back
    for (name, escapes, want) in [
        ("a click elsewhere in the host", 0, 1),
        ("Esc, then a click elsewhere", 1, 1),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut host = three(&dir);
        for _ in 0..escapes {
            press(&mut host, ShortcutKey::Escape);
        }
        host.click_elsewhere();
        assert_eq!(host.times(&PaneRequest::Unfocus), want, "{name}");
    }
}

#[test]
fn a_blur_while_the_host_has_not_given_the_keyboard_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = three(&dir);
    host.focus(false);
    host.click_elsewhere();
    assert!(!host.asked(&PaneRequest::Unfocus));
}
