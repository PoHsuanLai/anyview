//! Every kind of file the viewer claims, opened under the harness at 100% and 200% scale: what
//! shows, what the wheel and the arrow keys do over it, and the odd files (empty,
//! damaged, very wide) that a person will meet.

use crate::support;

use dioxus::prelude::Modifiers;
use ds::host::gesture::{Gesture, GesturePhase, ScrollSource};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::{Path, PathBuf};
use support::{Wiring, settle, title, wired};

const SCALES: [u16; 2] = [100, 200];

fn sample(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/formats")
        .join(name);
    std::fs::canonicalize(path).unwrap()
}

fn middle() -> Point {
    Point {
        x: Px(450.0),
        y: Px(300.0),
    }
}

fn opened(paths: &[PathBuf], scale_percent: u16) -> Harness {
    let viewport = Viewport {
        width: 900,
        height: 600,
        scale_percent,
    };
    let wiring = Wiring {
        viewport: Some(viewport),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    settle(&mut harness);
    harness.send(Input::pointer_move(middle()));
    settle(&mut harness);
    harness
}

#[test]
fn a_picture_wider_than_the_gpu_texture_limit_is_shown_not_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("panorama.png");
    image::RgbaImage::from_pixel(20_000, 200, image::Rgba([200, 30, 30, 255]))
        .save(&path)
        .unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    for scale in SCALES {
        let harness = opened(std::slice::from_ref(&path), scale);
        let failed = harness.text_of(".viewer-failed");
        assert!(
            failed.is_none() && harness.count(".viewer-raster") == 1,
            "STEP_FAIL a 20000x200 png at scale {scale}: expected the picture, got {failed:?}"
        );
    }
}

#[test]
fn the_wheel_under_command_zooms_a_picture_as_it_does_a_pdf() {
    for scale in SCALES {
        let mut harness = opened(&[sample("quadrants.png")], scale);
        let before = harness.text_of(".ds-capsule-readout");
        harness.send(Input::gesture(Gesture::Scroll {
            source: ScrollSource::Wheel,
            phase: GesturePhase::Changed,
            by: Point {
                x: Px(0.0),
                y: Px(40.0),
            },
            at: middle(),
            held: Modifiers::SUPER,
        }));
        settle(&mut harness);
        assert_ne!(
            before,
            harness.text_of(".ds-capsule-readout"),
            "STEP_FAIL Command+wheel over a picture at scale {scale} leaves the zoom at {before:?}"
        );
    }
}

/// The colour at the window's left edge, in the middle row.
fn columns_painted_at_the_left_edge(harness: &mut Harness, scale: u16) -> [u8; 3] {
    let image = harness.render().unwrap();
    let pixel = image.get_pixel(u32::from(scale) / 100, 300 * u32::from(scale) / 100);
    [pixel[0], pixel[1], pixel[2]]
}

#[test]
fn panning_a_zoomed_picture_to_its_end_brings_its_edge_flush_with_the_window() {
    for scale in SCALES {
        let mut harness = opened(&[sample("quadrants.png")], scale);
        for _ in 0..18 {
            harness.send(Input::key(ShortcutKey::Char('+')));
            settle(&mut harness);
        }
        // Fingers drag the picture to the right: its left edge comes into view and stops there.
        for _ in 0..200 {
            harness.send(Input::gesture(Gesture::Scroll {
                source: ScrollSource::Finger,
                phase: GesturePhase::Changed,
                by: Point {
                    x: Px(37.0),
                    y: Px(0.0),
                },
                at: middle(),
                held: Modifiers::empty(),
            }));
            settle(&mut harness);
        }
        let edge = columns_painted_at_the_left_edge(&mut harness, scale);
        let background = [236, 236, 236];
        assert_ne!(
            edge, background,
            "STEP_FAIL at scale {scale} the picture, panned fully right, can be dragged past its own edge, leaving background at the window's left edge: {edge:?}"
        );
    }
}

#[test]
fn a_zero_byte_picture_or_pdf_says_it_cannot_be_opened_not_an_empty_text() {
    for name in ["empty.png", "empty.pdf"] {
        for scale in SCALES {
            let harness = opened(&[sample(name)], scale);
            assert_eq!(
                harness.count(".viewer-text"),
                0,
                "STEP_FAIL {name} at scale {scale}: a zero-byte .{} shows as an empty text document",
                name.rsplit('.').next().unwrap()
            );
            let said = harness.text_of(".viewer-failed").unwrap_or_default();
            assert!(
                said.contains("empty"),
                "{name} at scale {scale}: the failed screen says {said:?}"
            );
        }
    }
}

#[test]
fn damaged_and_protected_files_say_what_is_wrong_in_words() {
    for (name, said) in [("corrupt.png", "damaged"), ("enc.pdf", "protected")] {
        for scale in SCALES {
            let harness = opened(&[sample(name)], scale);
            let text = harness.text_of(".viewer-failed").unwrap_or_default();
            assert!(text.contains(said), "{name} at {scale}: {text:?}");
        }
    }
}

#[test]
fn every_family_opens_to_its_own_stage_with_the_right_title() {
    for (name, stage) in [
        ("quadrants.png", ".viewer-raster"),
        ("cmyk.jpg", ".viewer-raster"),
        ("anim.gif", ".viewer-raster"),
        ("nosize.svg", ".viewer-raster"),
        ("multi.pdf", ".viewer-pdf"),
        ("r.md", ".viewer-text"),
        ("utf16le.txt", ".viewer-text"),
        ("ragged.csv", ".viewer-data"),
        ("a.json", ".viewer-node"),
        ("binaryish.txt", ".viewer-peek"),
    ] {
        for scale in SCALES {
            let harness = opened(&[sample(name)], scale);
            assert_eq!(title(&harness).as_deref(), Some(name), "{name} at {scale}");
            assert!(
                harness.count(stage) > 0 && harness.count(".viewer-failed") == 0,
                "STEP_FAIL {name} at scale {scale}: expected {stage}"
            );
        }
    }
}

#[test]
fn walking_a_mixed_folder_leaves_no_zoom_menu_or_stage_behind() {
    let files = [
        "quadrants.png",
        "anim.gif",
        "r.md",
        "multi.pdf",
        "book.epub",
    ];
    for scale in SCALES {
        let paths: Vec<PathBuf> = files.iter().map(|name| sample(name)).collect();
        let mut harness = opened(&paths, scale);
        harness.send(Input::key(ShortcutKey::Char('+')));
        settle(&mut harness);
        harness.send(Input::key(ShortcutKey::Right));
        settle(&mut harness);
        assert_eq!(title(&harness).as_deref(), Some("anim.gif"));
        assert_eq!(
            harness.attr(".viewer-raster", "data-zoom").as_deref(),
            Some("fit"),
            "STEP_FAIL scale {scale}: the next picture inherits the last one's zoom"
        );
        for expected in &files[2..] {
            harness.send(Input::key(ShortcutKey::Right));
            settle(&mut harness);
            assert_eq!(title(&harness).as_deref(), Some(*expected));
            assert_eq!(harness.count(".ds-menu"), 0);
            assert_eq!(
                harness.count(".viewer-raster"),
                0,
                "the {expected} window still holds the picture stage"
            );
        }
    }
}
