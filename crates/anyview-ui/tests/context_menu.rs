//! The right-click menu under the harness: a secondary click opens it at the pointer, its rows are
//! what the palette offers for the open file, a pick runs, Esc closes, and the chrome is held
//! while it is up.

#![allow(clippy::unwrap_used)]

#[path = "../../anyview-pdf/tests/support/mod.rs"]
mod pdf_fixture;
mod support;

use anyview_core::{Edit, FileAction, QuarterTurn};
use anyview_ui::{Command, EditRequest, HostRequest, StageCommand};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_core::press::PointerButton;
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use support::{
    Answer, FakeLine, FakePlayer, Requests, Wiring, folder, settle, text_file, window, wired,
};

const RULE: &str = "---";

fn at(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}

fn right_click(harness: &mut Harness, to: Point) {
    harness.send(Input::press(to, PointerButton::Secondary));
    settle(harness);
}

/// The menu's lines in order, a rule as `---`.
fn menu(harness: &Harness) -> Vec<String> {
    const LABEL: &str = "class=\"ds-menu-label\">";
    const RULE_MARK: &str = "class=\"ds-menu-separator\"";
    let html = harness.html();
    let mut found: Vec<(usize, String)> = html
        .match_indices(LABEL)
        .map(|(at, _)| {
            let from = at + LABEL.len();
            let to = html[from..].find('<').map_or(html.len(), |end| from + end);
            (at, html[from..to].to_owned())
        })
        .collect();
    found.extend(
        html.match_indices(RULE_MARK)
            .map(|(at, _)| (at, RULE.to_owned())),
    );
    found.sort();
    found.into_iter().map(|(_, line)| line).collect()
}

/// The palette's text for the open file.
fn palette(harness: &mut Harness) -> String {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    let text = harness.text_of(".ds-palette").unwrap_or_default();
    harness.send(Input::key(ShortcutKey::Escape));
    settle(harness);
    text
}

fn picture() -> (tempfile::TempDir, Harness, Requests) {
    let (dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let (mut harness, requests) = window(&paths, 0, Appearance::default());
    settle(&mut harness);
    (dir, harness, requests)
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
                    | HostRequest::Unwatch
            )
        })
        .cloned()
        .collect()
}

/// Every row of `lines` that is a command, checked against what the palette lists: the menu names
/// the same commands the palette does.
fn assert_the_palette_lists(lines: &[String], rows: &[(&str, Option<Command>)], listed: &str) {
    for (title, command) in rows {
        assert!(
            lines.iter().any(|line| line == title),
            "{title} in {lines:?}"
        );
        if let Some(command) = command {
            assert!(
                listed.contains(&command.label()),
                "{title}: the palette lists {}: {listed}",
                command.label()
            );
        }
    }
}

const fn file(action: FileAction) -> Option<Command> {
    Some(Command::File(action))
}

#[test]
fn a_right_click_opens_the_menu_at_the_pointer() {
    let (_dir, mut harness, _) = picture();
    assert_eq!(harness.count(".ds-menu"), 0, "no menu before the click");
    let pointer = at(300.0, 200.0);
    right_click(&mut harness, pointer);
    let rect = harness.rect(".ds-menu").expect("the menu is drawn");
    assert!(
        (rect.origin.x.0 - pointer.x.0).abs() <= 2.0
            && (rect.origin.y.0 - pointer.y.0).abs() <= 2.0,
        "the menu's corner {:?} is at the pointer {pointer:?}",
        rect.origin
    );
}

#[test]
fn a_picture_menu_lists_what_the_palette_offers_in_the_context_order() {
    let (_dir, mut harness, _) = picture();
    let listed = palette(&mut harness);
    right_click(&mut harness, at(300.0, 200.0));
    let lines = menu(&harness);
    assert_eq!(
        lines,
        [
            "Rotate Left",
            "Rotate Right",
            RULE,
            "Copy Path",
            RULE,
            "Open With\u{2026}",
            "Show in Folder",
            "Get Info",
            RULE,
            "Export\u{2026}",
            "Share\u{2026}",
            RULE,
            "Rename\u{2026}",
            "Duplicate",
            "Move to Trash",
        ]
    );
    assert_the_palette_lists(
        &lines,
        &[
            ("Rotate Left", file(FileAction::RotateLeft)),
            ("Rotate Right", file(FileAction::RotateRight)),
            ("Copy Path", file(FileAction::CopyPath)),
            ("Open With\u{2026}", file(FileAction::OpenWith)),
            ("Show in Folder", file(FileAction::RevealInFolder)),
            ("Export\u{2026}", file(FileAction::Export)),
            ("Share\u{2026}", file(FileAction::Share)),
            ("Rename\u{2026}", file(FileAction::Rename)),
            ("Duplicate", file(FileAction::Duplicate)),
            ("Move to Trash", file(FileAction::MoveToTrash)),
        ],
        &listed,
    );
}

#[test]
fn picking_rotate_runs_it_and_closes_the_menu() {
    let (_dir, mut harness, requests) = picture();
    right_click(&mut harness, at(300.0, 200.0));
    let before = asked(&requests).len();
    let rotate = harness.centre(".ds-menu-item").expect("a first row");
    harness.send(Input::click(rotate));
    // The row blinks, the menu runs it and fades out, then it closes.
    harness.advance(std::time::Duration::from_secs(1));
    let after = asked(&requests);
    assert_eq!(
        after[before..],
        [HostRequest::Edit(EditRequest::of_picture(Edit::Rotate(
            QuarterTurn::ThreeQuarter
        )))],
        "the first row is Rotate Left"
    );
    assert_eq!(
        harness.count(".ds-menu"),
        0,
        "the menu closed after the pick"
    );
}

#[test]
fn escape_closes_the_menu_and_runs_nothing() {
    let (_dir, mut harness, requests) = picture();
    right_click(&mut harness, at(300.0, 200.0));
    assert_eq!(harness.count(".ds-menu"), 1);
    let before = asked(&requests).len();
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-menu"), 0);
    assert_eq!(asked(&requests).len(), before);
}

#[test]
fn a_click_outside_closes_the_menu() {
    let (_dir, mut harness, requests) = picture();
    right_click(&mut harness, at(300.0, 200.0));
    let before = asked(&requests).len();
    harness.send(Input::click(at(800.0, 550.0)));
    settle(&mut harness);
    assert_eq!(harness.count(".ds-menu"), 0);
    assert_eq!(asked(&requests).len(), before);
}

#[test]
fn the_chrome_stays_while_the_menu_is_open_and_hides_after() {
    let (_dir, mut harness, _) = picture();
    // Idle long enough for the chrome to hide.
    harness.advance(std::time::Duration::from_secs(4));
    assert_eq!(
        harness.attr(".viewer-titlebar", "data-shown").as_deref(),
        Some("hidden")
    );
    right_click(&mut harness, at(300.0, 200.0));
    harness.advance(std::time::Duration::from_secs(4));
    assert_eq!(
        harness.attr(".viewer-titlebar", "data-shown").as_deref(),
        Some("visible"),
        "held by the open menu"
    );
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    harness.advance(std::time::Duration::from_secs(4));
    assert_eq!(
        harness.attr(".viewer-titlebar", "data-shown").as_deref(),
        Some("hidden"),
        "let go once the menu is closed"
    );
}

#[test]
fn the_menu_key_opens_it_at_the_middle_of_the_content() {
    let (_dir, mut harness, _) = picture();
    harness.send(Input::key(ShortcutKey::ContextMenu));
    settle(&mut harness);
    let rect = harness.rect(".ds-menu").expect("the menu is drawn");
    let stage = harness.rect(".viewer-stage").expect("the stage");
    let middle = (
        stage.origin.x.0 + stage.size.width.0 / 2.0,
        stage.origin.y.0 + stage.size.height.0 / 2.0,
    );
    // A menu too tall for the room below slides up to stay on screen.
    let bottom = rect.origin.y.0 + rect.size.height.0;
    let slid = bottom >= stage.origin.y.0 + stage.size.height.0 - 8.0;
    let at_middle = (rect.origin.y.0 - middle.1).abs() <= 2.0;
    assert!(
        (rect.origin.x.0 - middle.0).abs() <= 2.0 && (at_middle || slid),
        "the menu's corner {:?} is at the middle {middle:?}",
        rect.origin
    );
}

#[test]
fn a_text_file_menu_has_no_picture_rows() {
    let dir = tempfile::tempdir().unwrap();
    let notes = text_file(dir.path(), "notes.txt", "word", 40);
    let (mut harness, _) = window(&[notes], 0, Appearance::default());
    settle(&mut harness);
    let listed = palette(&mut harness);
    right_click(&mut harness, at(300.0, 200.0));
    let lines = menu(&harness);
    assert_eq!(
        lines,
        [
            "Copy Path",
            RULE,
            "Open With\u{2026}",
            "Show in Folder",
            "Get Info",
            RULE,
            "Export\u{2026}",
            "Share\u{2026}",
            RULE,
            "Rename\u{2026}",
            "Duplicate",
            "Move to Trash",
        ]
    );
    for hidden in ["Rotate", "Play"] {
        assert!(!lines.iter().any(|line| line.contains(hidden)), "{lines:?}");
        assert!(!listed.contains(&format!("{hidden} left")), "{listed}");
    }
}

fn pdf() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    let paths: Vec<PathBuf> = vec![std::fs::canonicalize(path).unwrap()];
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    settle(&mut harness);
    (dir, harness)
}

#[test]
fn a_pdf_menu_rotates_the_page_and_has_the_files_actions() {
    let (_dir, mut harness) = pdf();
    let listed = palette(&mut harness);
    right_click(&mut harness, at(300.0, 200.0));
    let lines = menu(&harness);
    assert_eq!(
        lines,
        [
            "Rotate Left",
            "Rotate Right",
            RULE,
            "Copy Path",
            RULE,
            "Open With\u{2026}",
            "Show in Folder",
            "Get Info",
            RULE,
            "Export\u{2026}",
            "Share\u{2026}",
            RULE,
            "Rename\u{2026}",
            "Duplicate",
            "Move to Trash",
        ]
    );
    assert_the_palette_lists(
        &lines,
        &[
            ("Rotate Left", file(FileAction::RotateLeft)),
            ("Export\u{2026}", file(FileAction::Export)),
        ],
        &listed,
    );
}

fn recording() -> (tempfile::TempDir, Harness, std::sync::Arc<FakePlayer>) {
    let (dir, paths) = folder(&[("anyview-media", "clip.mkv", "clip.mkv")]);
    let player = FakePlayer::answering(Answer::Plays);
    let wiring = Wiring {
        player: Some(std::sync::Arc::clone(&player)),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    (dir, harness, player)
}

fn loaded(line: &FakeLine) {
    use anyview_core::{MediaLength, MediaTime, VideoPresence};
    use anyview_ui::{MediaNotice, PlayerEvent};
    line.say(&[
        MediaNotice::Player(PlayerEvent::Loaded {
            length: MediaLength(MediaTime::from_secs(100)),
        }),
        MediaNotice::Position(MediaTime::from_secs(10)),
        MediaNotice::Picture(VideoPresence::Present),
    ]);
}

#[test]
fn a_recordings_menu_starts_with_playback_and_frame_steps() {
    let (_dir, mut harness, player) = recording();
    loaded(&player.latest().unwrap());
    settle(&mut harness);
    let listed = palette(&mut harness);
    right_click(&mut harness, at(300.0, 200.0));
    let lines = menu(&harness);
    assert_eq!(
        &lines[..4],
        ["Play/Pause", "Previous Frame", "Next Frame", RULE],
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("Rotate")),
        "{lines:?}"
    );
    assert_the_palette_lists(
        &lines,
        &[
            (
                "Play/Pause",
                Some(Command::Stage(StageCommand::TogglePlayback)),
            ),
            (
                "Previous Frame",
                Some(Command::Stage(StageCommand::StepFrameBack)),
            ),
            (
                "Next Frame",
                Some(Command::Stage(StageCommand::StepFrameForward)),
            ),
        ],
        &listed,
    );
}

#[test]
fn a_secondary_click_on_the_capsule_is_the_capsules_own() {
    let (_dir, mut harness, _) = picture();
    harness.send(Input::pointer_move(at(450.0, 300.0)));
    settle(&mut harness);
    let capsule = harness.centre(".ds-capsule").expect("the capsule");
    right_click(&mut harness, capsule);
    assert_eq!(harness.count(".ds-menu"), 0);
}

#[test]
fn a_read_only_picture_has_no_edit_rows_in_the_menu_or_the_palette() {
    let (_dir, paths) = folder(&[("anyview-image", "quadrants.png", "quadrants.png")]);
    let wiring = Wiring {
        locks: Some(std::sync::Arc::new(support::Locks(paths.clone()))),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    let listed = palette(&mut harness);
    right_click(&mut harness, at(300.0, 200.0));
    let lines = menu(&harness);
    assert_eq!(
        lines.first().map(String::as_str),
        Some("Copy Path"),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("Rotate")),
        "{lines:?}"
    );
    assert!(
        !listed.contains("Rotate"),
        "the palette hides the edits too: {listed}"
    );
    assert!(
        lines.contains(&"Duplicate".to_owned()),
        "a copy is still on offer: {lines:?}"
    );
}
