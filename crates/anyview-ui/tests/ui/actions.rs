//! What the window offers a person and what it does with it, under the harness: every action the
//! palette lists is bound or hidden, the sheets answer Return and Esc, Rename and Convert To open
//! their sheets, and what the host says of a task reaches the person as a notice.

use crate::support;

use anyview_core::{Edit, FileAction, FilePath, QuarterTurn};
use anyview_ui::{EditRequest, HostRequest, Notice, TypedText};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::time::Duration;
use support::{Requests, Wiring, folder, settle, text_file, window, wired};

fn picture() -> (tempfile::TempDir, Harness, Requests) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, requests) = window(&paths, 0, Appearance::default());
    settle(&mut harness);
    (dir, harness, requests)
}

fn palette(harness: &mut Harness, words: &str) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in words.chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
}

fn from_the_palette(harness: &mut Harness, words: &str) {
    palette(harness, words);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(harness);
}

fn asked(requests: &Requests) -> Vec<HostRequest> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| {
            !matches!(
                request,
                HostRequest::Opened(_)
                    | HostRequest::Watch(_)
                    | HostRequest::Remember(_)
                    | HostRequest::SizeWindow(_)
                    | HostRequest::Unwatch
            )
        })
        .cloned()
        .collect()
}

#[test]
fn the_palette_offers_only_what_applies_to_a_still_picture() {
    let (_dir, mut harness, _) = picture();
    palette(&mut harness, "");
    let listed = harness.text_of(".ds-palette").unwrap_or_default();
    for offered in ["Rename", "Convert to", "Duplicate", "Print"] {
        assert!(listed.contains(offered), "{offered} is listed: {listed}");
    }
    for hidden in [
        "Copy file",
        "Toggle playback",
        "Step frame",
        "Seek",
        "mini window",
        "trim",
    ] {
        assert!(
            !listed.contains(hidden),
            "{hidden} does nothing for a still picture: {listed}"
        );
    }
}

#[test]
fn every_shortcut_the_palette_shows_for_a_file_action_runs_that_action() {
    // name, the modifiers, the key, what the window asks of its host
    let cases: Vec<(&str, Vec<ShortcutKey>, char, HostRequest)> = vec![
        (
            "print",
            vec![ShortcutKey::Ctrl],
            'p',
            HostRequest::Run(FileAction::Print),
        ),
        (
            "duplicate",
            vec![ShortcutKey::Ctrl],
            'd',
            HostRequest::Run(FileAction::Duplicate),
        ),
        (
            "rotate right",
            vec![ShortcutKey::Ctrl],
            ']',
            HostRequest::Edit(EditRequest::of_picture(Edit::Rotate(QuarterTurn::Quarter))),
        ),
        (
            "rotate left",
            vec![ShortcutKey::Ctrl],
            '[',
            HostRequest::Edit(EditRequest::of_picture(Edit::Rotate(
                QuarterTurn::ThreeQuarter,
            ))),
        ),
        (
            "reveal",
            vec![ShortcutKey::Ctrl],
            'r',
            HostRequest::Run(FileAction::RevealInFolder),
        ),
        (
            "copy the path",
            vec![ShortcutKey::Ctrl, ShortcutKey::Alt],
            'c',
            HostRequest::Run(FileAction::CopyPath),
        ),
    ];
    for (name, modifiers, key, want) in cases {
        let (_dir, mut harness, requests) = picture();
        harness.send(Input::chord(&modifiers, ShortcutKey::Char(key)));
        settle(&mut harness);
        assert_eq!(asked(&requests), [want], "{name}");
    }
}

#[test]
fn the_command_key_is_the_one_the_desktop_maps_to_command() {
    // Command and Control are one key to the viewer: the platform layer resolves it.
    let (_dir, mut harness, requests) = picture();
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('d')));
    settle(&mut harness);
    assert_eq!(asked(&requests), [HostRequest::Run(FileAction::Duplicate)]);
}

#[test]
fn rename_opens_its_sheet_on_the_current_name_and_return_confirms_it() {
    let (_dir, mut harness, requests) = picture();
    from_the_palette(&mut harness, "rename");
    assert_eq!(harness.count(".viewer-sheet"), 1, "the rename sheet is up");
    assert!(asked(&requests).is_empty(), "nothing is asked yet");
    harness.send(Input::key(ShortcutKey::Enter));
    settle(&mut harness);
    assert_eq!(
        asked(&requests),
        [HostRequest::Rename(TypedText::new("quadrants.png"))],
        "Return confirms the sheet"
    );
    assert_eq!(harness.count(".viewer-sheet"), 0, "and puts it away");
}

#[test]
fn escape_cancels_a_sheet_and_asks_for_nothing() {
    let (_dir, mut harness, requests) = picture();
    from_the_palette(&mut harness, "rename");
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-sheet"), 0);
    assert!(asked(&requests).is_empty());
}

#[test]
fn return_confirms_the_export_and_the_save_a_copy_sheets() {
    // name, what is typed, what the window asks of its host
    for (name, words, wanted) in [
        ("export", "export", "Export"),
        ("convert to", "convert", "Export"),
        ("save a copy", "save copy", "SaveCopy"),
    ] {
        let (_dir, mut harness, requests) = picture();
        from_the_palette(&mut harness, words);
        assert_eq!(harness.count(".viewer-sheet"), 1, "{name}: the sheet is up");
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        let asked = asked(&requests);
        assert_eq!(asked.len(), 1, "{name}: {asked:?}");
        assert!(
            format!("{:?}", asked[0]).starts_with(wanted),
            "{name}: {asked:?}"
        );
        assert_eq!(harness.count(".viewer-sheet"), 0, "{name}");
    }
}

#[test]
fn what_the_host_says_of_a_task_is_a_toast_and_only_a_file_offers_the_folder() {
    // row, whether the notice names a file to reveal, the words of the notice, the words the toast shows
    for (row, revealing, said, shown) in [
        (
            "an export",
            true,
            "Exported as \u{201c}quadrants 2.png\u{201d}",
            "Exported as",
        ),
        (
            "a failure with no file",
            false,
            "Couldn\u{2019}t move the file to the Trash",
            "Trash",
        ),
    ] {
        let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
        let (mut harness, requests, edge) =
            wired(&paths, 0, Appearance::default(), Wiring::default());
        settle(&mut harness);
        let made = FilePath::new(dir.path().join("quadrants 2.png")).unwrap();
        let notice = Notice::say(said);
        edge.notify(if revealing {
            notice.revealing(made.clone())
        } else {
            notice
        });
        harness.advance(Duration::from_millis(600));
        let toast = harness.text_of(".ds-toast").unwrap_or_default();
        assert!(toast.contains(shown), "row {row}: {toast}");
        if revealing {
            assert!(toast.contains("Show in Folder"), "row {row}: {toast}");
            let button = harness.centre(".ds-toast-action").expect("the action");
            harness.send(Input::click(button));
            settle(&mut harness);
            assert_eq!(asked(&requests), [HostRequest::Reveal(made)], "row {row}");
        } else {
            assert_eq!(harness.count(".ds-toast-action"), 0, "row {row}: no button");
        }
    }
}

#[test]
fn a_file_the_host_renamed_is_shown_under_its_new_name() {
    let dir = tempfile::tempdir().unwrap();
    let old = text_file(dir.path(), "before.txt", "line", 3);
    let (mut harness, _, edge) = wired(
        std::slice::from_ref(&old),
        0,
        Appearance::default(),
        Wiring::default(),
    );
    settle(&mut harness);
    assert_eq!(support::title(&harness).as_deref(), Some("before.txt"));
    let new = dir.path().join("after.txt");
    std::fs::rename(&old, &new).unwrap();
    edge.moved(FilePath::new(std::fs::canonicalize(new).unwrap()).unwrap());
    settle(&mut harness);
    assert_eq!(support::title(&harness).as_deref(), Some("after.txt"));
}
