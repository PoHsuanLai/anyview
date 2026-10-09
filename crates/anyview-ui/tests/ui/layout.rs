//! Layout at every size and scale under the harness: the chrome fits at each window size and scale,
//! survives a live resize, and what opens (the menu, the palette, the panel) stays on screen.
use crate::pdf_fixture;
use crate::support;
use anyview_core::{MediaLength, MediaTime, VideoPresence};
use anyview_ui::{MediaNotice, PlayerEvent};
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_blitz::Extent;
use ds_harness::{DocQuery, Driver, Harness, Input, Query, Viewport};
use std::time::Duration;
use support::{Answer, FakePlayer, Wiring, folder, wired};

const LONG_PNG: &str = "a-very-long-holiday-picture-name-that-keeps-on-going-and-going-2026.png";

pub fn settle(h: &mut Harness) {
    h.advance(Duration::from_millis(400));
}

fn open(kind: &str, w: u32, ht: u32, scale: u32) -> (tempfile::TempDir, Harness) {
    let viewport = Some(Viewport {
        width: w,
        height: ht,
        scale_percent: scale as u16,
    });
    let mut wiring = Wiring {
        viewport,
        ..Wiring::default()
    };
    let (dir, paths) = match kind {
        "picture" => folder(&[("anyview-image", "quadrants.png", LONG_PNG)]),
        "text" => folder(&[("anyview-text", "notes.txt", "n.txt")]),
        "table" => folder(&[("anyview-text", "people.csv", "p.csv")]),
        "media" => {
            wiring.player = Some(FakePlayer::answering(Answer::Plays));
            folder(&[("anyview-media", "clip.mkv", "c.mkv")])
        }
        "pdf" => {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().join("f.pdf");
            std::fs::write(&p, pdf_fixture::fixture_bytes()).unwrap();
            let p = std::fs::canonicalize(p).unwrap();
            (d, vec![p])
        }
        _ => panic!(),
    };
    let player = wiring.player.clone();
    let (mut h, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut h);
    if let Some(p) = player {
        p.latest().unwrap().say(&[
            MediaNotice::Player(PlayerEvent::Loaded {
                length: MediaLength(MediaTime::from_secs(100)),
            }),
            MediaNotice::Position(MediaTime::from_secs(25)),
            MediaNotice::Picture(VideoPresence::Present),
        ]);
        settle(&mut h);
    }
    (dir, h)
}

#[derive(Clone, Copy, Debug)]
struct Box4 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn all(h: &Harness, sel: &str) -> Vec<Box4> {
    h.with_doc(|doc| {
        doc.query_selector_all(sel)
            .map(|ids| {
                ids.into_iter()
                    .filter_map(|id| doc.get_client_bounding_rect(id))
                    .map(|r| Box4 {
                        x: r.x as f32,
                        y: r.y as f32,
                        w: r.width as f32,
                        h: r.height as f32,
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

fn pointer(h: &mut Harness, w: u32, ht: u32) {
    h.send(Input::pointer_move(Point {
        x: Px(w as f32 / 2.0),
        y: Px(ht as f32 / 2.0),
    }));
    settle(h);
}

/// Everything wrong with the chrome at this window size.
fn audit(h: &Harness, w: u32, ht: u32) -> Vec<String> {
    let (w, ht) = (w as f32, ht as f32);
    let mut bad = Vec::new();
    for sel in [
        ".viewer-titlebar",
        ".ds-titlebar-name",
        ".ds-lights",
        ".ds-capsule",
        ".ds-capsule .ds-button",
        ".ds-capsule-readout",
        ".viewer-stage",
        ".viewer-raster-picture",
    ] {
        for b in all(h, sel) {
            if b.w <= 0.0 || b.h <= 0.0 {
                bad.push(format!("{sel} empty {b:?}"));
            }
            if b.x < -0.5 || b.x + b.w > w + 0.5 {
                bad.push(format!("{sel} overflows x {b:?} in {w}"));
            }
            if sel != ".viewer-raster-picture" && (b.y < -0.5 || b.y + b.h > ht + 0.5) {
                bad.push(format!("{sel} overflows y {b:?} in {ht}"));
            }
        }
    }
    if let (Some(n), Some(t)) = (
        all(h, ".ds-titlebar-name").first().copied(),
        all(h, ".viewer-titlebar").first().copied(),
    ) {
        if n.x + n.w > t.x + t.w + 0.5 {
            bad.push(format!("title name past titlebar {n:?} {t:?}"));
        }
        if let Some(l) = all(h, ".ds-lights").first()
            && n.x < l.x + l.w - 0.5
        {
            bad.push(format!("title name over the lights {n:?} {l:?}"));
        }
    }
    let cap = all(h, ".ds-capsule");
    if let Some(c) = cap.first() {
        let mid = c.x + c.w / 2.0;
        if (mid - w / 2.0).abs() > 1.5 {
            bad.push(format!("capsule off-centre {c:?} in {w}"));
        }
    }
    bad
}

const WIDTHS: [u32; 9] = [336, 449, 497, 622, 672, 760, 1000, 1280, 1920];

#[test]
fn the_chrome_fits_at_every_size_and_scale() {
    let mut fails = std::collections::BTreeMap::<String, Vec<String>>::new();
    for kind in ["picture", "pdf", "text", "table", "media"] {
        let widths: &[u32] = if matches!(kind, "picture" | "text" | "table") {
            &WIDTHS
        } else {
            &[336, 497, 672, 1920]
        };
        for scale in [100, 200] {
            for ht in [300, 600] {
                for &w in widths {
                    let (_d, mut h) = open(kind, w, ht, scale);
                    pointer(&mut h, w, ht);
                    let bad = audit(&h, w, ht);
                    if !bad.is_empty() {
                        fails
                            .entry(kind.to_string())
                            .or_default()
                            .push(format!("{w}x{ht}@{scale}: {}", bad.join(" | ")));
                    }
                }
            }
        }
    }
    assert!(
        fails.is_empty(),
        "STEP_FAIL|L-sweep|the chrome fits every size → {fails:#?}"
    );
}

fn resize(h: &mut Harness, w: u32, ht: u32) {
    h.resize_window(Extent::new(w, ht));
    settle(h);
    pointer(h, w, ht);
}

fn zoom_text(h: &Harness) -> Option<String> {
    h.text_of(".ds-capsule-readout")
}

fn centre_of(b: Box4) -> Point {
    Point {
        x: Px(b.x + b.w / 2.0),
        y: Px(b.y + b.h / 2.0),
    }
}

#[test]
fn a_window_resized_down_and_back_keeps_its_zoom_and_the_capsule_hits_where_it_is_drawn() {
    for scale in [100, 200] {
        let (_d, mut h) = open("picture", 900, 600, scale);
        pointer(&mut h, 900, 600);
        h.send(Input::key(ShortcutKey::Char('+')));
        settle(&mut h);
        let zoomed = zoom_text(&h);
        for (w, ht) in [(336, 300), (1920, 1200), (449, 300), (900, 600)] {
            resize(&mut h, w, ht);
            assert_eq!(
                zoom_text(&h),
                zoomed,
                "L-live@{scale} {w}x{ht}: the zoom survives"
            );
            let bad = audit(&h, w, ht);
            assert!(bad.is_empty(), "L-live@{scale} {w}x{ht}: {bad:?}");
            let pic = all(&h, ".viewer-raster-picture")[0];
            let c = centre_of(pic);
            assert!(
                (c.x.0 - w as f32 / 2.0).abs() <= 1.5 && (c.y.0 - ht as f32 / 2.0).abs() <= 1.5,
                "L-live@{scale} {w}x{ht}: the picture is centred {pic:?}"
            );
            let b = all(&h, ".ds-capsule .ds-button")[1];
            h.send(Input::click(centre_of(b)));
            settle(&mut h);
            assert_ne!(
                zoom_text(&h),
                zoomed,
                "L-hit@{scale} {w}x{ht}: the zoom-in button, as drawn, zooms"
            );
            h.send(Input::key(ShortcutKey::Char('0')));
            h.send(Input::key(ShortcutKey::Char('+')));
            settle(&mut h);
            assert_eq!(zoom_text(&h), zoomed);
        }
    }
}

#[test]
fn the_titlebar_does_not_cover_the_top_of_the_panel_tabs() {
    let (_d, mut h) = open("pdf", 612, 792, 100);
    pointer(&mut h, 612, 792);
    h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut h);
    pointer(&mut h, 612, 792);
    assert_eq!(
        h.attr(".viewer-titlebar", "data-shown").as_deref(),
        Some("visible")
    );
    let bar = all(&h, ".viewer-titlebar")[0];
    let tabs = all(&h, ".ds-segmented")[0];
    assert!(
        tabs.y >= bar.y + bar.h,
        "STEP_FAIL|L-panel-titlebar|the tabs start below the titlebar's bottom {} → they start at {}|pdf 612x792@100, the info chord, move the pointer",
        bar.y + bar.h,
        tabs.y
    );
}
