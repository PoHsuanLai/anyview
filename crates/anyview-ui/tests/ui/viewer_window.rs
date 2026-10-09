//! The viewer window under the harness on the virtual clock: what shows by default, what the
//! pointer brings and takes away, what ⌘K lists, and what the arrow keys walk.

use crate::support;

use anyview_core::{FileAction, Reach, actions_for, reach};
use anyview_ui::ChromeParams;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Input, Query};
use std::time::Duration;
use support::{VIEW, folder, shot, window};

const FILES: &[(&str, &str, &str)] = &[
    ("anyview-text", "notes.txt", "1-notes.txt"),
    ("anyview-text", "sample.rs", "2-sample.rs"),
    ("anyview-text", "readme.md", "3-readme.md"),
];

fn middle() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

fn title(harness: &impl Query) -> Option<String> {
    harness.text_of(".ds-titlebar-name")
}

#[test]
fn only_the_content_shows_until_the_pointer_moves_and_the_chrome_leaves_when_it_rests() {
    let (_dir, paths) = folder(FILES);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    let hidden = |harness: &ds_harness::Harness| {
        (
            harness.attr(".viewer-titlebar", "data-shown"),
            harness.attr(".ds-capsule", "data-shown"),
        )
    };
    assert_eq!(
        hidden(&harness),
        (Some("hidden".to_owned()), Some("hidden".to_owned())),
        "nothing but the content at first"
    );
    assert!(
        harness
            .text_of(".viewer-text")
            .is_some_and(|text| text.contains("1")),
        "the file's lines are on screen"
    );

    harness.send(Input::pointer_move(middle()));
    harness.advance(ChromeParams::QUICK + Duration::from_millis(10));
    assert_eq!(
        hidden(&harness),
        (Some("visible".to_owned()), Some("visible".to_owned())),
        "the pointer brings the titlebar and the capsule"
    );

    let rest = ChromeParams::default().hide_after;
    harness.advance(rest - Duration::from_millis(100));
    assert_eq!(
        harness.attr(".viewer-titlebar", "data-shown").as_deref(),
        Some("visible"),
        "still there just before the idle time is up"
    );
    harness.advance(Duration::from_millis(100) + ChromeParams::QUICK * 2);
    assert_eq!(
        hidden(&harness),
        (Some("hidden".to_owned()), Some("hidden".to_owned())),
        "and gone once it is"
    );
}

#[test]
fn command_k_lists_every_viewer_action_of_the_file() {
    let (_dir, paths) = folder(FILES);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    assert_eq!(harness.count(".ds-palette"), 0, "closed to begin with");
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    harness.advance(Duration::from_millis(300));
    let text = harness.text_of(".ds-palette").unwrap_or_default();
    let shared: Vec<FileAction> = actions_for(anyview_core::FormatKind::PlainText)
        .iter()
        .copied()
        // Copy File waits for a clipboard that holds a file, so the palette does not list it.
        .filter(|action| *action != FileAction::CopyFile)
        .filter(|action| match reach(*action) {
            Reach::Viewer | Reach::Both => true,
            Reach::Launcher => false,
        })
        .collect();
    assert!(!shared.is_empty(), "a text file has viewer actions");
    for action in shared {
        assert!(
            text.contains(ds_core_label(action)),
            "{action:?} is listed in {text:?}"
        );
    }
    harness.send(Input::key(ShortcutKey::Escape));
    harness.advance(Duration::from_millis(300));
    assert_eq!(harness.count(".ds-palette"), 0, "Esc closes it");
}

fn ds_core_label(action: FileAction) -> &'static str {
    use ds::prelude::Word;
    action.label()
}

#[test]
fn the_arrow_keys_walk_the_folder_and_stop_at_its_ends() {
    let (_dir, paths) = folder(FILES);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.send(Input::pointer_move(middle()));
    harness.advance(Duration::from_millis(300));
    assert_eq!(title(&harness).as_deref(), Some("1-notes.txt"));
    for (key, want) in [
        (ShortcutKey::Right, "2-sample.rs"),
        (ShortcutKey::Right, "3-readme.md"),
        (ShortcutKey::Right, "3-readme.md"),
        (ShortcutKey::Left, "2-sample.rs"),
    ] {
        harness.send(Input::key(key));
        harness.advance(Duration::from_millis(300));
        assert_eq!(title(&harness).as_deref(), Some(want), "after {key:?}");
    }
    if let Some(path) = shot("walked.png") {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn the_markup_the_window_renders_lints_clean() {
    let (_dir, paths) = folder(FILES);
    let (mut harness, _) = window(&paths, 0, Appearance::default());
    harness.send(Input::pointer_move(middle()));
    harness.advance(Duration::from_millis(300));
    let css = format!("{}\n{}", ds::stylesheet(), anyview_ui::stylesheet());
    let config = ds_lint::LintConfig::new(&ds::kits());
    let shown = ds_lint::markup(&harness.html(), &css, &config);
    assert!(shown.is_empty(), "chrome shown: {shown:#?}");
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    harness.advance(Duration::from_millis(300));
    let palette = ds_lint::markup(&harness.html(), &css, &config);
    assert!(palette.is_empty(), "palette open: {palette:#?}");
}
