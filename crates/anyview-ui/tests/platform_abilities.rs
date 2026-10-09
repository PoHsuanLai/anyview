//! What a window offers when the platform lacks a desktop service, under the harness: with every
//! ability the viewer is as it always was, and with none the actions that need a service (a file
//! chooser, a print dialog, sharing, a list of applications, a file manager) are not in the
//! palette, not in the context menu, not on the failure and card screens, and their keys do
//! nothing. The welcome window has no Open… without a file chooser.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FileAction;
use anyview_ui::{Edge, HostRequest, PlatformAbilities, WelcomeApp, Workers};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_core::press::PointerButton;
use ds_harness::{Backend, Clock, Driver, Harness, HarnessConfig, Input, Query};
use std::sync::Arc;
use std::time::Duration;
use support::{Requests, VIEW, Wiring, folder, settle, wired};

/// The labels of the actions that need a desktop service, as the palette words them.
const DESKTOP: &[&str] = &["Share", "Print", "Reveal in folder"];

fn with(platform: PlatformAbilities, files: &[(&str, &str, &str)]) -> (Harness, Requests) {
    let (dir, paths) = folder(files);
    let wiring = Wiring {
        platform: Some(platform),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    // The folder lives as long as the harness does.
    std::mem::forget(dir);
    (harness, requests)
}

fn picture(platform: PlatformAbilities) -> (Harness, Requests) {
    with(
        platform,
        &[("anyview-image", "quadrants.png", "quadrants.png")],
    )
}

fn palette(harness: &mut Harness) -> String {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    let text = harness.text_of(".ds-palette").unwrap_or_default();
    harness.send(Input::key(ShortcutKey::Escape));
    settle(harness);
    text
}

/// The labels of the context menu's rows.
fn context_menu(harness: &mut Harness) -> Vec<String> {
    harness.send(Input::press(
        Point {
            x: Px(300.0),
            y: Px(200.0),
        },
        PointerButton::Secondary,
    ));
    settle(harness);
    const LABEL: &str = "class=\"ds-menu-label\">";
    let html = harness.html();
    let labels = html
        .match_indices(LABEL)
        .map(|(at, _)| {
            let from = at + LABEL.len();
            let to = html[from..].find('<').map_or(html.len(), |end| from + end);
            html[from..to].to_owned()
        })
        .collect();
    harness.send(Input::key(ShortcutKey::Escape));
    settle(harness);
    labels
}

fn chord(harness: &mut Harness, modifiers: &[ShortcutKey], key: char) {
    harness.send(Input::chord(modifiers, ShortcutKey::Char(key)));
    settle(harness);
}

fn asked(requests: &Requests) -> Vec<HostRequest> {
    requests.lock().unwrap().clone()
}

#[test]
fn with_every_ability_the_palette_the_menu_and_the_keys_are_as_they_were() {
    let (mut harness, requests) = picture(PlatformAbilities::ALL);
    let listed = palette(&mut harness);
    for label in DESKTOP {
        assert!(listed.contains(label), "{label} is listed: {listed}");
    }
    let menu = context_menu(&mut harness);
    for label in ["Share\u{2026}", "Show in Folder"] {
        assert!(menu.iter().any(|row| row == label), "{label} in {menu:?}");
    }
    chord(&mut harness, &[ShortcutKey::Ctrl], 'o');
    chord(&mut harness, &[ShortcutKey::Ctrl], 'p');
    let asked = asked(&requests);
    assert!(asked.contains(&HostRequest::PickFile), "⌘O asks for a file");
    assert!(
        asked.contains(&HostRequest::Run(FileAction::Print)),
        "⌘P prints"
    );
}

#[test]
fn with_no_ability_the_desktop_actions_are_in_no_palette_no_menu_and_on_no_key() {
    let (mut harness, requests) = picture(PlatformAbilities::NONE);
    let listed = palette(&mut harness);
    for label in DESKTOP {
        assert!(!listed.contains(label), "{label} is hidden: {listed}");
    }
    for kept in ["Rename", "Duplicate", "Convert to"] {
        assert!(listed.contains(kept), "{kept} is still listed: {listed}");
    }
    let menu = context_menu(&mut harness);
    for label in ["Share\u{2026}", "Show in Folder"] {
        assert!(!menu.iter().any(|row| row == label), "{label} in {menu:?}");
    }
    assert!(
        menu.iter().any(|row| row == "Move to Trash"),
        "the rest of the menu stays: {menu:?}"
    );
    chord(&mut harness, &[ShortcutKey::Ctrl], 'o');
    chord(&mut harness, &[ShortcutKey::Ctrl], 'p');
    chord(&mut harness, &[ShortcutKey::Ctrl], 'r');
    let asked = asked(&requests);
    for request in [
        HostRequest::PickFile,
        HostRequest::Run(FileAction::Print),
        HostRequest::Run(FileAction::RevealInFolder),
    ] {
        assert!(
            !asked.contains(&request),
            "{request:?} has no key: {asked:?}"
        );
    }
}

/// An archive with nothing in it: a card, with Show in Folder.
fn empty_zip() -> Vec<u8> {
    let mut zip = b"PK\x05\x06".to_vec();
    zip.extend([0; 18]);
    zip
}

fn card(platform: PlatformAbilities) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.zip");
    std::fs::write(&path, empty_zip()).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    let wiring = Wiring {
        platform: Some(platform),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    std::mem::forget(dir);
    harness
}

#[test]
fn a_card_draws_the_buttons_of_the_services_there_are_and_none_of_those_there_are_not() {
    // name, the abilities, the buttons that remain
    let cases = [
        ("all", PlatformAbilities::ALL, 1),
        (
            "no file manager",
            PlatformAbilities {
                reveal: false,
                ..PlatformAbilities::ALL
            },
            0,
        ),
        ("none", PlatformAbilities::NONE, 0),
    ];
    for (name, platform, buttons) in cases {
        let harness = card(platform);
        assert!(harness.count(".viewer-peek") > 0, "{name}: the card shows");
        assert_eq!(
            harness.count(".viewer-peek .viewer-failed-actions .ds-button"),
            buttons,
            "{name}"
        );
    }
}

fn welcome(platform: PlatformAbilities) -> (Harness, Requests) {
    let requests: Requests = Arc::default();
    let seen = Arc::clone(&requests);
    let workers: Arc<dyn Workers> = Arc::new(support::Inline);
    let edge = Edge::new(workers, move |request| seen.lock().unwrap().push(request))
        .with_platform(platform);
    let config = HarnessConfig::new(VIEW)
        .with_clock(Clock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_context(edge);
    let mut harness = Harness::new(WelcomeApp, config);
    harness.advance(Duration::from_millis(500));
    (harness, requests)
}

#[test]
fn the_welcome_window_asks_for_a_file_only_where_there_is_a_chooser() {
    let (mut harness, requests) = welcome(PlatformAbilities::ALL);
    assert_eq!(
        harness.count(".ds-empty-state-action .ds-button"),
        1,
        "Open… is there"
    );
    let at = harness.centre(".ds-empty-state-action .ds-button").unwrap();
    harness.send(Input::click(at));
    settle(&mut harness);
    assert!(asked(&requests).contains(&HostRequest::PickFile));

    let (mut harness, requests) = welcome(PlatformAbilities {
        pick_files: false,
        ..PlatformAbilities::ALL
    });
    assert_eq!(
        harness.count(".ds-empty-state-action .ds-button"),
        0,
        "no Open…"
    );
    let said = harness.text_of(".viewer-welcome").unwrap_or_default();
    assert!(!said.contains("Choose"), "{said:?}");
    chord(&mut harness, &[ShortcutKey::Ctrl], 'o');
    assert!(!asked(&requests).contains(&HostRequest::PickFile), "no ⌘O");
}

const OPEN: &str = "Open\u{2026}";

#[test]
fn open_is_in_the_palette_and_the_menu_where_there_is_a_chooser() {
    let (mut harness, _) = picture(PlatformAbilities::ALL);
    let listed = palette(&mut harness);
    assert!(listed.contains(OPEN), "Open… is listed: {listed}");
    let menu = context_menu(&mut harness);
    let open = menu
        .iter()
        .position(|row| row == OPEN)
        .expect("Open… in the menu");
    let reveal = menu
        .iter()
        .position(|row| row == "Show in Folder")
        .expect("Show in Folder in the menu");
    assert_eq!(
        open + 1,
        reveal,
        "Open… sits just before Show in Folder: {menu:?}"
    );
}

#[test]
fn open_is_in_neither_the_palette_nor_the_menu_without_a_chooser() {
    let (mut harness, _) = picture(PlatformAbilities {
        pick_files: false,
        ..PlatformAbilities::ALL
    });
    let listed = palette(&mut harness);
    assert!(!listed.contains(OPEN), "{listed}");
    let menu = context_menu(&mut harness);
    assert!(!menu.iter().any(|row| row == OPEN), "{menu:?}");
}
