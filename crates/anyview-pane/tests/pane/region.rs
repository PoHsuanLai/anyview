//! A pane is the file in a region of the host's window: it fills the region, draws none of the
//! window's own chrome, and several share one pool.

use crate::support::{hosted, image_file, text_file, walking};
use anyview_pane::{FilePath, LoadFailure, NotOpened, PaneRequest};
use ds_harness::{Query, Viewport};

/// What a window draws around its file and a pane must not.
const WINDOW_CHROME: [&str; 5] = [
    ".viewer-titlebar",
    ".ds-titlebar-name",
    ".viewer-welcome",
    ".ds-palette",
    ".ds-sheet",
];

#[test]
fn a_pane_fills_its_region_and_draws_none_of_the_windows_chrome() {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        text_file(dir.path(), "notes.txt", "word", 200),
        image_file(dir.path(), "picture.png", "quadrants.png"),
    ];
    // The region's logical size and the scale it is drawn at.
    for (width, height, scale_percent) in [(480, 320, 100), (300, 200, 100), (480, 320, 200)] {
        for file in &files {
            let viewport = Viewport {
                width,
                height,
                scale_percent,
            };
            let host = walking(std::slice::from_ref(file), 0, viewport);
            let name = file.file_name().unwrap().to_str().unwrap();
            let label = format!("{name} in {width}x{height} at {scale_percent}%");
            assert_eq!(host.showing().as_deref(), Some(name), "{label}: it opened");
            let region = host.harness.rect(".viewer").expect("the pane");
            assert!(
                (region.size.width.0 - width as f32).abs() < 1.0
                    && (region.size.height.0 - height as f32).abs() < 1.0,
                "{label}: the pane is {:?}",
                region.size
            );
            for chrome in WINDOW_CHROME {
                assert_eq!(host.harness.count(chrome), 0, "{label}: {chrome}");
            }
        }
    }
}

#[test]
fn two_panes_share_one_pool_and_each_shows_its_own_file() {
    let dir = tempfile::tempdir().unwrap();
    let left = text_file(dir.path(), "left.txt", "alpha", 50);
    let right = text_file(dir.path(), "right.txt", "bravo", 50);
    let viewport = Viewport {
        width: 900,
        height: 400,
        scale_percent: 100,
    };
    let host = hosted(vec![(left, None), (right, None)], viewport);
    let mut shown = host.shown();
    shown.sort();
    assert_eq!(shown, ["left.txt", "right.txt"]);
    assert_eq!(host.harness.count(".viewer"), 2);
    assert_eq!(
        host.harness.count(".viewer-text"),
        2,
        "each pane draws its text"
    );
    assert!(
        host.workers.submitted() >= 2,
        "both panes' work went to the one pool: {}",
        host.workers.submitted()
    );
}

#[test]
fn a_file_that_cannot_be_opened_is_told_to_the_host_once_and_never_as_opened() {
    let dir = tempfile::tempdir().unwrap();
    // A picture by its name with no bytes in it: the one failure that needs no decoder.
    let empty = dir.path().join("empty.png");
    std::fs::write(&empty, b"").unwrap();
    let empty = std::fs::canonicalize(empty).unwrap();
    let viewport = Viewport {
        width: 480,
        height: 320,
        scale_percent: 100,
    };
    let mut host = hosted(vec![(empty.clone(), None)], viewport);
    host.settle();
    let not_opened = PaneRequest::NotOpened(NotOpened {
        file: FilePath::new(&empty).unwrap(),
        reason: LoadFailure::Empty,
    });
    assert_eq!(host.times(&not_opened), 1, "once, not again on a redraw");
    assert!(host.shown().is_empty(), "no file was announced as opened");
    assert_eq!(host.harness.count(".viewer-failed"), 1, "the card says so");
}
