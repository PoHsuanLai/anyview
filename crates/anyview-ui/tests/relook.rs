//! A new look from the desktop re-themes a window that is already open: the feed the program
//! hands every window changes, and the window's own colours follow, with no reopening.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_ui::{Look, LookFeed};
use ds::prelude::{Appearance, Theme};
use ds_harness::{Driver, Harness};
use std::time::Duration;
use support::{VIEW, Wiring, folder, wired};

fn look(theme: Theme) -> Look {
    Look::from(Appearance {
        theme,
        ..Appearance::default()
    })
}

/// How light the window's corner is, 0 to 255: the ground the file sits on.
fn ground(harness: &mut Harness) -> u32 {
    let image = harness.render().unwrap();
    let pixel = image.get_pixel(VIEW.width - 4, VIEW.height - 4);
    (u32::from(pixel.0[0]) + u32::from(pixel.0[1]) + u32::from(pixel.0[2])) / 3
}

#[test]
fn a_new_look_re_themes_the_open_window_and_a_window_without_a_feed_keeps_its_own() {
    let (_dir, paths) = folder(&[("anyview-text", "notes.txt", "a.txt")]);
    let (tx, rx) = tokio::sync::watch::channel(look(Theme::Light));
    let wiring = Wiring {
        feed: Some(LookFeed(rx)),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    harness.advance(Duration::from_millis(500));
    let light = ground(&mut harness);

    tx.send(look(Theme::Dark)).unwrap();
    harness.advance(Duration::from_millis(800));
    let dark = ground(&mut harness);
    assert!(light > dark + 60, "light {light}, then dark {dark}");

    tx.send(look(Theme::Light)).unwrap();
    harness.advance(Duration::from_millis(800));
    assert!(ground(&mut harness) > dark + 60, "back to light");

    let pinned = Appearance {
        theme: Theme::Dark,
        ..Appearance::default()
    };
    let (mut own, _, _) = wired(&paths, 0, pinned, Wiring::default());
    own.advance(Duration::from_millis(500));
    assert!(
        light > ground(&mut own) + 60,
        "no feed: the launch look stays"
    );
}
