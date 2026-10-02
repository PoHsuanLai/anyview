//! The scheduler as tables: a stack of Letter pages, a window, and the tiles expected in order.

use super::*;
use crate::geometry::{MilliPoints, PageSize};
use crate::layout::PageLayout;
use anyview_core::{PageIndex, Permille, PixelLen, PixelSize};

const LETTER: PageSize = PageSize {
    width: MilliPoints(612_000),
    height: MilliPoints(792_000),
};

fn stack(scale: u32, pages: usize) -> PageLayout {
    PageLayout::new(&vec![LETTER; pages], Permille(scale), PixelLen(8))
}

fn window(page: u32, left: i32, width: u32, height: u32) -> ViewWindow {
    ViewWindow {
        page: PageIndex(page),
        offset: Permille(0),
        left,
        size: PixelSize {
            width: PixelLen(width),
            height: PixelLen(height),
        },
    }
}

/// `(page, column, row)` of each key.
fn cells(keys: &[TileKey]) -> Vec<(u32, u32, u32)> {
    keys.iter().map(|key| (key.page.0, key.x, key.y)).collect()
}

#[test]
fn a_window_wants_its_tiles_nearest_the_middle_first_then_the_margin() {
    // name, layout scale, pages, window, margin, visible, preload; cells are (page, column, row).
    type Row = (
        &'static str,
        u32,
        usize,
        ViewWindow,
        u32,
        &'static [(u32, u32, u32)],
        &'static [(u32, u32, u32)],
    );
    let cases: [Row; 3] = [
        (
            "top of the document, page centred in a wider window",
            1000,
            3,
            window(0, -94, 800, 600),
            600,
            &[(0, 0, 0), (0, 1, 0), (0, 0, 1), (0, 1, 1)],
            &[(1, 0, 0), (1, 1, 0)],
        ),
        (
            "second page at the top, neighbours around it",
            1000,
            3,
            window(1, -94, 800, 600),
            600,
            &[(1, 0, 0), (1, 1, 0), (1, 0, 1), (1, 1, 1)],
            &[
                (0, 0, 1),
                (0, 1, 1),
                (2, 0, 0),
                (0, 0, 0),
                (2, 1, 0),
                (0, 1, 0),
            ],
        ),
        (
            "no margin, nothing preloaded",
            1000,
            3,
            window(0, -94, 800, 300),
            0,
            &[(0, 0, 0), (0, 1, 0)],
            &[],
        ),
    ];
    for (name, scale, pages, win, margin, visible, preload) in cases {
        let got = schedule(&stack(scale, pages), &win, PixelLen(margin));
        assert_eq!(cells(got.visible()), visible, "{name}: visible");
        assert_eq!(cells(got.preload()), preload, "{name}: preload");
    }
}

#[test]
fn a_zoomed_page_is_cut_to_the_part_in_view() {
    // 2000 permille: the page is 1224 x 1584 pixels; the window shows x 500 to 1500, y 0 to 800.
    let got = schedule(&stack(2000, 1), &window(0, 500, 1000, 800), PixelLen(0));
    assert_eq!(got.zoom().scale(), Permille(2000));
    assert_eq!(
        cells(got.visible()),
        [
            (0, 2, 0),
            (0, 1, 0),
            (0, 2, 1),
            (0, 1, 1),
            (0, 0, 0),
            (0, 0, 1)
        ]
    );
    assert!(got.preload().is_empty());
}

#[test]
fn a_zoom_between_buckets_draws_at_the_next_one_up() {
    let got = schedule(&stack(1500, 1), &window(0, 0, 400, 400), PixelLen(0));
    assert_eq!(got.zoom().scale(), Permille(1682));
    assert!(got.keys().all(|key| key.zoom == got.zoom()));
    assert_eq!(cells(got.visible()), [(0, 0, 0)]);
}

#[test]
fn a_window_past_the_end_wants_only_the_margin_of_the_last_page() {
    let layout = stack(1000, 2);
    let got = schedule(&layout, &window(9, 0, 612, 100), PixelLen(100));
    assert!(got.visible().is_empty());
    assert!(!got.preload().is_empty());
    assert!(got.keys().all(|key| key.page == PageIndex(1)));
}

#[test]
fn what_is_drawn_is_not_asked_for_again() {
    let wanted = schedule(&stack(1000, 3), &window(1, -94, 800, 600), PixelLen(600));
    let drawn = wanted.visible()[0];
    let rest = wanted.missing(|key| *key == drawn);
    assert_eq!(rest.visible().len(), wanted.visible().len() - 1);
    assert_eq!(rest.preload(), wanted.preload());
    assert!(wanted.wants(&drawn));
    assert!(!rest.keys().any(|key| *key == drawn));
}

#[test]
fn jobs_are_one_batch_per_page_visible_first() {
    let wanted = schedule(&stack(1000, 3), &window(1, -94, 800, 600), PixelLen(600));
    let got: Vec<(u32, Priority, usize)> = wanted
        .batches()
        .iter()
        .map(|batch| (batch.page.0, batch.priority, batch.tiles.len()))
        .collect();
    assert_eq!(
        got,
        [
            (1, Priority::Visible, 4),
            (0, Priority::Preload, 4),
            (2, Priority::Preload, 2),
        ]
    );
    let first = &wanted.batches()[0];
    assert_eq!(first.tiles[0], TileCoord { x: 0, y: 0 });
    assert!(wanted.batches().iter().all(|batch| !batch.tiles.is_empty()));
}
