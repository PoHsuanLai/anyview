//! A PDF in the viewer window under the harness: it opens, its tiles arrive as the workers answer
//! and show as texture layers, ⌘F finds text and the next hit moves the page, a link goes where it
//! points, and the zoom follows a pinch and the wheel under Control. Workers run each job where it
//! is submitted, so an answer is in the mailbox by the time the harness looks.

use crate::pdf_fixture;
use crate::support;

use anyview_core::{PageIndex, Permille, Resume, Zoom};
use anyview_ui::HostRequest;
use dioxus::prelude::Modifiers;
use ds::host::gesture::{Gesture, GesturePhase, Magnification, ScrollSource};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query};
use std::path::PathBuf;
use std::time::Duration;
use support::{Memory, Requests, VIEW, Wiring, shot, window, wired};

/// The fixture written to a scratch folder: three pages, a find target on two of them, links.
fn fixture() -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    (dir, vec![std::fs::canonicalize(path).unwrap()])
}

fn opened() -> (tempfile::TempDir, Harness, Requests) {
    let (dir, paths) = fixture();
    let (mut harness, requests) = window(&paths, 0, Appearance::default());
    harness.advance(Duration::from_millis(500));
    (dir, harness, requests)
}

fn middle() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

/// What the capsule reads out: the page and the zoom.
fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

fn saved(harness: &mut Harness, name: &str) {
    if let Some(path) = shot(name) {
        harness.render().unwrap().save(path).unwrap();
    }
}

fn settle(harness: &mut Harness) {
    harness.advance(Duration::from_millis(300));
}

fn type_text(harness: &mut Harness, text: &str) {
    for c in text.chars() {
        harness.send(Input::key(ShortcutKey::Char(c)));
    }
    settle(harness);
}

#[test]
fn a_pdf_opens_with_its_pages_laid_out_and_its_tiles_drawn() {
    let (_dir, mut harness, _) = opened();
    assert!(
        harness.count(".viewer-pdf-page") >= 1,
        "the first page is in the room"
    );
    assert!(
        harness.count(".viewer-pdf-tile") >= 1,
        "its tiles arrived from the workers and are layers"
    );
    assert!(capsule(&harness).contains("1 / 3"), "{}", capsule(&harness));
    saved(&mut harness, "pdf-opened.png");
}

#[test]
fn command_f_finds_text_and_the_next_hit_moves_the_page() {
    let (_dir, mut harness, _) = opened();
    assert_eq!(harness.count(".viewer-find"), 0, "no find bar until asked");
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('f')));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-find"), 1, "the bar is up");
    type_text(&mut harness, "fox");
    // "fox" is once on page 1 and twice on page 2: three hits, the one nearest the reader first.
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("1 of 3")
    );
    assert!(capsule(&harness).contains("1 / 3"), "{}", capsule(&harness));
    assert!(
        harness.count(".viewer-pdf-hit") >= 1,
        "the hit is marked over the page"
    );
    saved(&mut harness, "pdf-find.png");

    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('g')));
    settle(&mut harness);
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("2 of 3")
    );
    assert!(
        capsule(&harness).contains("2 / 3"),
        "the next hit is on page 2: {}",
        capsule(&harness)
    );
    saved(&mut harness, "pdf-find-next.png");

    harness.send(Input::chord(
        &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        ShortcutKey::Char('g'),
    ));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("1 / 3"),
        "and the previous is back on page 1: {}",
        capsule(&harness)
    );

    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-find"), 0, "Esc closes the find");
    assert_eq!(
        harness.count(".viewer-pdf-hit"),
        0,
        "and takes the marks away"
    );
}

#[test]
fn keys_typed_in_the_find_field_are_text_and_not_commands_of_the_stage() {
    let (_dir, mut harness, _) = opened();
    let before = capsule(&harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('f')));
    settle(&mut harness);
    // `9` fits the width and `+` zooms in when a stage hears them.
    type_text(&mut harness, "9+");
    assert_eq!(
        harness.text_of(".viewer-find-standing").as_deref(),
        Some("No matches"),
        "the keys were typed into the query"
    );
    assert_eq!(capsule(&harness), before, "and moved nothing");
}

#[test]
fn a_link_to_a_page_goes_there_and_a_web_link_goes_to_the_host() {
    let (_dir, mut harness, requests) = opened();
    assert!(
        harness.count(".viewer-pdf-link") >= 2,
        "the page's links are over it"
    );
    let links = harness.centre(".viewer-pdf-link").unwrap();
    harness.send(Input::click(links));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("3 / 3"),
        "the first link leads to page 3: {}",
        capsule(&harness)
    );
    harness.send(Input::key(ShortcutKey::PageUp));
    harness.send(Input::key(ShortcutKey::PageUp));
    settle(&mut harness);
    let web = harness.centre(".viewer-pdf-link:nth-last-child(1)");
    assert!(
        web.is_some(),
        "back on page 1 the web link is the last of its links"
    );
    harness.send(Input::click(web.unwrap()));
    settle(&mut harness);
    assert!(
        requests
            .lock()
            .unwrap()
            .contains(&HostRequest::OpenUri("https://example.com/".to_owned())),
        "{:?}",
        requests.lock().unwrap()
    );
}

#[test]
fn a_pinch_and_the_wheel_under_control_zoom_and_the_wheel_alone_scrolls() {
    let (_dir, mut harness, _) = opened();
    let zoom_of = |harness: &Harness| {
        capsule(harness)
            .split('%')
            .next()
            .and_then(|head| head.rsplit(char::is_whitespace).next().map(str::to_owned))
            .unwrap_or_default()
    };
    let at = middle();
    let before = capsule(&harness);
    harness.send(Input::gesture(Gesture::Pinch {
        phase: GesturePhase::Changed,
        by: Magnification(300),
        at,
    }));
    settle(&mut harness);
    let pinched = capsule(&harness);
    assert_ne!(before, pinched, "a pinch changes the zoom");
    harness.send(Input::gesture(Gesture::Scroll {
        source: ScrollSource::Wheel,
        phase: GesturePhase::Changed,
        by: Point {
            x: Px(0.0),
            y: Px(40.0),
        },
        at,
        held: Modifiers::CONTROL,
    }));
    settle(&mut harness);
    assert_ne!(pinched, capsule(&harness), "the wheel under Control zooms");
    let _ = zoom_of;
    saved(&mut harness, "pdf-zoomed.png");

    harness.send(Input::key(ShortcutKey::Char('0')));
    settle(&mut harness);
    let fitted = capsule(&harness);
    harness.send(Input::wheel(at, Px(0.0), Px(-700.0)));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("2 / 3"),
        "the wheel scrolls to the next page: {fitted} then {}",
        capsule(&harness)
    );
}

#[test]
fn the_panel_lists_the_pages_and_the_outline_and_each_goes_to_its_page() {
    let (_dir, mut harness, _) = opened();
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    harness.send(Input::click(
        harness.centre(".ds-segmented-segment").unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-thumb"),
        3,
        "a thumbnail for each page"
    );
    saved(&mut harness, "pdf-panel.png");
    harness.send(Input::click(
        harness
            .centre(".viewer-thumbs .ds-row:nth-child(3)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(capsule(&harness).contains("3 / 3"), "{}", capsule(&harness));
    harness.send(Input::click(
        harness
            .centre(".viewer-thumbs .ds-row:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(capsule(&harness).contains("2 / 3"), "{}", capsule(&harness));

    // The outline is the second tab: four bookmarks, each to its page.
    let tabs = |harness: &Harness, at: usize| {
        harness
            .centre(&format!(".ds-segmented-segment:nth-child({at})"))
            .unwrap()
    };
    harness.send(Input::click(tabs(&harness, 2)));
    settle(&mut harness);
    assert_eq!(
        harness.count(".viewer-outline-entry"),
        4,
        "the fixture's bookmarks"
    );
    saved(&mut harness, "pdf-outline.png");
    harness.send(Input::click(
        harness
            .centre(".viewer-outline-entry:nth-child(4)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("3 / 3"),
        "Appendix: {}",
        capsule(&harness)
    );
}

#[test]
fn dragging_the_page_scrolls_it_with_the_pointer() {
    let (_dir, mut harness, _) = opened();
    // The first page is 568 pixels tall at the fit and the gap 12, so a drag of 590 up takes the
    // second page to the top of the room.
    let from = Point {
        x: Px(450.0),
        y: Px(595.0),
    };
    let to = Point {
        x: from.x,
        y: Px(5.0),
    };
    assert!(capsule(&harness).contains("1 / 3"));
    harness.send(Input::drag(from, to, 8));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("2 / 3"),
        "dragging up moves the next page into view: {}",
        capsule(&harness)
    );
}

#[test]
fn the_markup_of_a_pdf_with_a_find_bar_lints_clean() {
    let (_dir, mut harness, _) = opened();
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('f')));
    settle(&mut harness);
    let css = format!("{}\n{}", ds::stylesheet(), anyview_ui::stylesheet());
    let config = ds_lint::LintConfig::new(&ds::kits());
    // quire's `TextureLayer` styles itself inline and its class has no rule (FINDINGS, open item
    // for quire), so the lint flags quire's own component and that one selector is let through.
    let offences: Vec<_> = ds_lint::markup(&harness.html(), &css, &config)
        .into_iter()
        .filter(|offence| offence.selector != "object.ds-texture-layer")
        .collect();
    assert!(offences.is_empty(), "{offences:#?}");
}

#[test]
fn a_pdf_opens_on_the_page_the_store_remembers() {
    let (_dir, paths) = fixture();
    let memory = Memory::with(
        &paths[0],
        Resume::Pdf {
            page: PageIndex(2),
            offset: Permille(0),
            zoom: Zoom::Fit,
        },
    );
    let wiring = Wiring {
        memory: Some(memory),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("3 / 3"),
        "it opened where it was left: {}",
        capsule(&harness)
    );
}

#[test]
fn end_and_home_go_to_the_last_page_and_the_first_and_a_line_key_scrolls_in_the_page() {
    let (_dir, mut harness, _) = opened();
    harness.send(Input::key(ShortcutKey::End));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("3 / 3"),
        "End is the last page: {}",
        capsule(&harness)
    );
    harness.send(Input::key(ShortcutKey::Home));
    settle(&mut harness);
    assert!(
        capsule(&harness).contains("1 / 3"),
        "Home is the first: {}",
        capsule(&harness)
    );
    let before = harness.centre(".viewer-pdf-page");
    harness.send(Input::key(ShortcutKey::Down));
    settle(&mut harness);
    assert_ne!(
        harness.centre(".viewer-pdf-page"),
        before,
        "a line down moved the page up"
    );
}

#[test]
fn a_pdf_capsule_fits_a_window_at_its_least_width() {
    let (_dir, paths) = fixture();
    let (mut harness, _, _) = wired(
        &paths,
        0,
        Appearance::default(),
        Wiring {
            viewport: Some(ds_harness::Viewport {
                width: 480,
                height: 320,
                scale_percent: 100,
            }),
            ..Wiring::default()
        },
    );
    harness.advance(Duration::from_millis(500));
    harness.send(Input::pointer_move(Point {
        x: Px(240.0),
        y: Px(160.0),
    }));
    settle(&mut harness);
    let capsule = harness.rect(".ds-capsule").expect("the capsule shows");
    assert!(
        capsule.origin.x.0 >= 0.0 && capsule.origin.x.0 + capsule.size.width.0 <= 480.0,
        "the capsule is inside the window: {capsule:?}"
    );
    assert_eq!(
        harness.count(".ds-capsule .ds-button"),
        7,
        "all seven buttons at 480"
    );
}
