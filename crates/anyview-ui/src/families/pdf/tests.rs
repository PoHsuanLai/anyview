//! The PDF stage's arithmetic and bookkeeping as tables: where the pages are, what the room asks
//! of the scheduler, where a tile, a hit and a link go, which tiles a cache lets go, and what the
//! live state does with a plan and an answer. No window and no GPU.

use super::cache::evictions;
use super::live::PdfLive;
use super::scene::{Frame, Scene, drawn, fits, percent, pinched, scale_of};
use super::work::{Finish, FlightId, PdfAnswer};
use crate::{FindOut, PdfOut, Stage, StageIn, TypedText};
use anyview_core::work::StopState;
use anyview_core::{PageIndex, Permille, Zoom};
use anyview_pdf::Hits;
use anyview_pdf::{MilliPoints, PageRect, PageSize, Priority, TileKey, ZoomBucket};

const fn points(width: u32, height: u32) -> PageSize {
    PageSize {
        width: MilliPoints(width * 1000),
        height: MilliPoints(height * 1000),
    }
}

const LETTER: PageSize = points(612, 792);
const FRAME: Frame = Frame {
    width: 900,
    height: 600,
    density: Permille(1000),
};

fn stack(sizes: &[PageSize], scale: u32) -> Scene {
    Scene::new(sizes, FRAME, Permille(scale))
}

const fn rect(left: u32, top: u32, right: u32, bottom: u32) -> PageRect {
    PageRect {
        left: Permille(left),
        top: Permille(top),
        right: Permille(right),
        bottom: Permille(bottom),
    }
}

#[test]
fn a_zoom_means_a_scale_for_this_window_and_this_document() {
    // name, pages, frame (width, height, density), zoom, expected scale
    type Row = (
        &'static str,
        &'static [PageSize],
        (u32, u32, u32),
        Zoom,
        u32,
    );
    const CASES: &[Row] = &[
        // 900 - 2 * 16 = 868 wide over 612 pt; 600 - 32 = 568 high over 792 pt.
        ("fit width", &[LETTER], (900, 600, 1000), Zoom::Fill, 1418),
        (
            "fit page is the tighter of the two",
            &[LETTER],
            (900, 600, 1000),
            Zoom::Fit,
            717,
        ),
        (
            "actual size",
            &[LETTER],
            (900, 600, 1000),
            Zoom::Actual,
            1000,
        ),
        (
            "a fixed scale",
            &[LETTER],
            (900, 600, 1000),
            Zoom::Scale(Permille(2500)),
            2500,
        ),
        (
            "the widest page sets the width",
            &[LETTER, points(1224, 792)],
            (900, 600, 1000),
            Zoom::Fill,
            709,
        ),
        (
            "the margin is logical so a dense screen keeps it",
            &[LETTER],
            (1800, 1200, 2000),
            Zoom::Fill,
            2836,
        ),
    ];
    for (name, sizes, (width, height, density), zoom, want) in CASES {
        let frame = Frame {
            width: *width,
            height: *height,
            density: Permille(*density),
        };
        let got = scale_of(*zoom, fits(sizes, frame));
        assert_eq!(got, Permille(*want), "{name}");
    }
}

#[test]
fn a_pinch_scales_and_stays_inside_the_zoom_limits() {
    // name, scale, thousandths, expected
    const CASES: &[(&str, u32, i32, u32)] = &[
        ("grows", 1000, 250, 1250),
        ("shrinks", 1000, -250, 750),
        ("no change", 1000, 0, 1000),
        ("not below the floor", 12, -900, 10),
        ("not above the ceiling", 60_000, 4000, 64_000),
        ("a huge pinch is bounded", 1000, i32::MAX, 5000),
    ];
    for (name, scale, by, want) in CASES {
        assert_eq!(pinched(Permille(*scale), *by), Permille(*want), "{name}");
    }
}

#[test]
fn a_page_and_offset_name_the_top_of_the_room_and_back() {
    let scene = stack(&[LETTER, LETTER, LETTER], 1000);
    // Each page is 792 tall with a 12 pixel gap.
    assert_eq!(scene.top_of(PageIndex(1), Permille(500)), 804 + 396);
    assert_eq!(scene.place_at(804 + 396), (PageIndex(1), Permille(500)));
    // The room cannot go past the foot of the last page.
    let foot = 3 * 792 + 2 * 12 - 600;
    assert_eq!(scene.deepest(), u64::try_from(foot).unwrap_or(0));
    assert_eq!(scene.top_of(PageIndex(2), Permille(900)), scene.deepest());
}

#[test]
fn pages_narrower_than_the_room_are_centred_and_wider_ones_pan() {
    // name, scale, pan, where the stack's left edge is
    const CASES: &[(&str, u32, u32, i64)] = &[
        ("centred", 1000, 0, -144),
        ("pan is ignored while centred", 1000, 99, -144),
        ("panned", 2000, 100, 100),
        ("pan stops at the right edge", 2000, 9999, 1224 - 900),
    ];
    for (name, scale, pan, want) in CASES {
        let scene = stack(&[LETTER], *scale);
        assert_eq!(scene.left_of(*pan), *want, "{name}");
    }
}

#[test]
fn the_room_asks_for_the_tiles_it_shows_nearest_first_and_a_margin_after() {
    let scene = stack(&[LETTER, LETTER], 1000);
    let schedule = scene.schedule(0, 0);
    let bucket = ZoomBucket::containing(Permille(1000));
    assert_eq!(schedule.zoom(), bucket);
    let visible = schedule.visible();
    assert!(!visible.is_empty(), "the room shows something");
    assert!(
        visible.iter().all(|key| key.page == PageIndex(0)),
        "only the first page is in view at the top"
    );
    // 612 x 792 at 1:1 is 2 columns by 2 rows of 512 pixel tiles, and the room (600 high) shows
    // the first row and a sliver of the second.
    assert_eq!(visible.len(), 4, "{visible:?}");
    let batches = schedule.batches();
    assert_eq!(
        batches.first().map(|batch| batch.priority),
        Some(Priority::Visible)
    );
    assert!(
        schedule
            .preload()
            .iter()
            .any(|key| key.page == PageIndex(1)),
        "the page below is read ahead"
    );
}

#[test]
fn scrolling_changes_which_tiles_are_asked_for() {
    let scene = stack(&[LETTER, LETTER, LETTER], 1000);
    let at_top = scene.schedule(0, 0);
    let down = scene.schedule(1700, 0);
    assert_ne!(at_top.visible(), down.visible());
    assert!(
        down.visible().iter().any(|key| key.page == PageIndex(2)),
        "{:?}",
        down.visible()
    );
}

#[test]
fn a_tile_goes_where_its_pixels_are_on_the_page() {
    // name, sizes, scale of the stack, zoom bucket step scale, key (page, x, y), expected span
    // (left, top, right, bottom) in the stack
    let scene = stack(&[LETTER, LETTER], 1000);
    let bucket = ZoomBucket::containing(Permille(1000));
    assert_eq!(bucket.scale(), Permille(1000));
    let first = scene
        .tile_span(&TileKey {
            page: PageIndex(0),
            zoom: bucket,
            x: 0,
            y: 0,
        })
        .unwrap();
    assert_eq!(
        (first.left, first.top, first.right, first.bottom),
        (0, 0, 512, 512)
    );
    // The edge tile is cut to the page, not to the extra pixel the renderer draws.
    let edge = scene
        .tile_span(&TileKey {
            page: PageIndex(0),
            zoom: bucket,
            x: 1,
            y: 1,
        })
        .unwrap();
    assert_eq!(
        (edge.left, edge.top, edge.right, edge.bottom),
        (512, 512, 612, 792)
    );
    // The second page starts after the first and the gap.
    let second = scene
        .tile_span(&TileKey {
            page: PageIndex(1),
            zoom: bucket,
            x: 0,
            y: 0,
        })
        .unwrap();
    assert_eq!(second.top, 792 + 12);
    // A tile of a coarser zoom covers the same page area with fewer, larger pixels.
    let coarse = ZoomBucket::containing(Permille(500));
    let big = scene
        .tile_span(&TileKey {
            page: PageIndex(0),
            zoom: coarse,
            x: 0,
            y: 0,
        })
        .unwrap();
    assert_eq!((big.left, big.top, big.right), (0, 0, 612));
    assert_eq!(
        big.bottom, 792,
        "the whole page fits one tile at half scale"
    );
}

#[test]
fn the_old_zoom_shows_under_the_new_one_until_the_new_tiles_are_there() {
    let scene = stack(&[LETTER], 1000);
    let fine = ZoomBucket::containing(Permille(1000));
    let coarse = ZoomBucket::containing(Permille(500));
    let old = TileKey {
        page: PageIndex(0),
        zoom: coarse,
        x: 0,
        y: 0,
    };
    let view = scene.view(0, 0);
    let keys_over = |zoom| scene.covering(PageIndex(0), scene.tile_span(&old).unwrap(), zoom);
    let wanted = keys_over(fine);
    assert_eq!(
        wanted.len(),
        4,
        "the fine zoom needs four tiles for what one coarse tile held"
    );

    // Nothing of the fine zoom yet: the coarse tile is drawn.
    assert_eq!(drawn(&scene, view, fine, &[old]), vec![old]);
    // Some of them: the coarse tile still shows under them.
    let partial: Vec<TileKey> = std::iter::once(old)
        .chain(wanted.iter().take(2).copied())
        .collect();
    let got = drawn(&scene, view, fine, &partial);
    assert_eq!(got.first(), Some(&old), "the old tile is under");
    assert_eq!(got.len(), 3);
    // All of them: the coarse tile is not drawn at all.
    let full: Vec<TileKey> = std::iter::once(old).chain(wanted.iter().copied()).collect();
    let got = drawn(&scene, view, fine, &full);
    assert_eq!(got.len(), 4);
    assert!(!got.contains(&old));
}

#[test]
fn a_rectangle_of_a_page_is_a_percentage_of_its_box() {
    // name, rectangle, (left, top, width, height) in percent
    type Row = (&'static str, PageRect, (f32, f32, f32, f32));
    const CASES: &[Row] = &[
        (
            "the whole page",
            rect(0, 0, 1000, 1000),
            (0.0, 0.0, 100.0, 100.0),
        ),
        ("a word", rect(118, 180, 497, 200), (11.8, 18.0, 37.9, 2.0)),
        (
            "a reversed rectangle has no size",
            rect(500, 500, 400, 400),
            (50.0, 50.0, 0.0, 0.0),
        ),
    ];
    for (name, placed, want) in CASES {
        assert_eq!(percent(*placed), *want, "{name}");
    }
}

#[test]
fn a_hit_is_shown_a_third_of_the_way_down_and_never_above_its_page() {
    let scene = stack(&[LETTER, LETTER, LETTER], 1000);
    // name, page, rectangles, expected top of the room
    let cases: &[(&str, u32, &[PageRect], Option<u64>)] = &[
        // Page 2 starts at 804; the hit is 18% down (142), so a third of the room (200) above it
        // would be above the page: the room's top is the page's.
        (
            "near the top of a page",
            1,
            &[rect(100, 180, 400, 200)],
            Some(804),
        ),
        // 60% down page 2: 804 + 475 = 1279, less 200.
        (
            "lower on the page",
            1,
            &[rect(100, 600, 400, 620)],
            Some(1079),
        ),
        ("a page that is not there", 9, &[rect(0, 0, 1, 1)], None),
        ("a hit with no rectangle", 1, &[], None),
    ];
    for (name, page, rects, want) in cases {
        assert_eq!(scene.top_for_hit(PageIndex(*page), rects), *want, "{name}");
    }
}

#[test]
fn zooming_keeps_the_point_under_the_pointer_where_it_was() {
    let before = stack(&[LETTER, LETTER], 1000);
    let after = before.at_scale(Permille(2000));
    // The pointer is at (450, 300) in the room, which is at the top of the stack, centred: the
    // room's left edge is 144 left of the stack's.
    let (top, pan) = after.anchored(&before, 0, 0, (450, 300));
    // The stack point under the pointer was (-144 + 450, 300) = (306, 300); doubled it is
    // (612, 600), so the room's top is 300 and its left edge 162.
    assert_eq!((top, pan), (300, 162));
}

#[test]
fn the_cache_lets_go_of_the_far_and_the_old_before_the_near_and_never_the_wanted() {
    let key = |page: u32, step: i8| TileKey {
        page: PageIndex(page),
        zoom: ZoomBucket::containing(Permille(
            u32::try_from(1000 + i32::from(step) * 250).unwrap_or(1000),
        )),
        x: 0,
        y: 0,
    };
    let current = ZoomBucket::containing(Permille(1000));
    let held: Vec<(TileKey, u64)> = vec![
        (key(5, 0), 100),
        (key(6, 0), 100),
        (key(0, 0), 100),
        (key(5, 2), 100),
        (key(5, -2), 100),
    ];
    // name, budget, wanted pages, expected let go (by page and step), in order
    let wanted_5 = |k: &TileKey| k.page == PageIndex(5) && k.zoom == current;
    let none = |_: &TileKey| false;
    type Row<'a> = (&'a str, u64, &'a dyn Fn(&TileKey) -> bool, Vec<TileKey>);
    let cases: Vec<Row<'_>> = vec![
        ("under budget lets go of nothing", 500, &none, vec![]),
        (
            "one over lets go of the farthest zoom",
            400,
            &none,
            vec![key(5, 2)],
        ),
        (
            "two over adds the next farthest zoom",
            300,
            &none,
            vec![key(5, 2), key(5, -2)],
        ),
        (
            "a wanted tile stays",
            100,
            &wanted_5,
            vec![key(5, 2), key(5, -2), key(0, 0), key(6, 0)],
        ),
    ];
    for (name, budget, wanted, want) in cases {
        let got = evictions(&held, budget, current, PageIndex(5), wanted);
        let mut got_sorted = got.clone();
        got_sorted.sort();
        let mut want_sorted = want.clone();
        want_sorted.sort();
        assert_eq!(got_sorted, want_sorted, "{name}");
    }
}

fn key_at(page: u32, x: u32, y: u32) -> TileKey {
    TileKey {
        page: PageIndex(page),
        zoom: ZoomBucket::containing(Permille(1000)),
        x,
        y,
    }
}

#[test]
fn a_plan_asks_for_what_is_missing_once_and_ends_what_nothing_wants() {
    let mut live = PdfLive::default();
    let scene = stack(&[LETTER, LETTER, LETTER], 1000);
    let top = scene.schedule(0, 0);
    let first = live.plan(&top, PageIndex(0));
    assert!(!first.is_empty());
    assert_eq!(
        first[0].batch.priority,
        Priority::Visible,
        "the visible tiles go first"
    );
    let asked: usize = first.iter().map(|asked| asked.batch.tiles.len()).sum();
    assert_eq!(asked, top.visible().len() + top.preload().len());
    assert!(
        first.iter().all(|asked| asked.batch.tiles.len() <= 6),
        "jobs are short"
    );

    // The same room again asks for nothing: everything is in flight.
    assert!(live.plan(&top, PageIndex(0)).is_empty());

    // A room far away wants none of those, so they are stopped and its own are asked for.
    let far = scene.schedule(1700, 0);
    let second = live.plan(&far, PageIndex(2));
    assert!(!second.is_empty());
    assert!(
        first
            .iter()
            .all(|asked| asked.stop.stopped() == StopState::Stopped
                || far.keys().any(|key| key.page == asked.batch.page)),
        "a batch nothing wants is told to stop"
    );
}

#[test]
fn a_tile_that_cannot_be_drawn_is_not_asked_for_again() {
    let mut live = PdfLive::default();
    let scene = stack(&[LETTER], 1000);
    let schedule = scene.schedule(0, 0);
    let asked = live.plan(&schedule, PageIndex(0));
    let bad = key_at(0, 0, 0);
    let flight = asked[0].flight;
    let input = live.received(
        PdfAnswer::Tiles {
            flight,
            ready: Vec::new(),
            unfinished: Vec::new(),
            failed: vec![bad],
        },
        &Stage::NoStage,
    );
    assert_eq!(input, None);
    let again = live.plan(&schedule, PageIndex(0));
    assert!(
        again
            .iter()
            .all(|asked| !asked.batch.tiles.iter().any(|at| at.x == 0
                && at.y == 0
                && asked.batch.page == PageIndex(0)
                && asked.batch.zoom == bad.zoom)),
        "{again:?}"
    );
}

#[test]
fn a_search_answers_the_machine_once_and_only_for_the_query_asked() {
    let mut live = PdfLive::default();
    let query = TypedText::new("fox");
    let _stop = live.searching(query.clone());
    let answer = |text: &str, end| PdfAnswer::Searched {
        query: TypedText::new(text),
        hits: Hits::default(),
        end,
    };
    // name, answer, whether the machine is told
    let cases = [
        ("the query asked", answer("fox", Finish::Complete), true),
        ("another query", answer("dog", Finish::Complete), false),
        ("a search cut short", answer("fox", Finish::Cut), false),
    ];
    for (name, reply, told) in cases {
        let input = live.received(reply, &Stage::NoStage);
        assert_eq!(input.is_some(), told, "{name}");
    }
}

#[test]
fn a_newer_search_stops_the_one_before_it() {
    let mut live = PdfLive::default();
    let before = live.searching(TypedText::new("fo"));
    assert_eq!(before.stopped(), StopState::Running);
    let _after = live.searching(TypedText::new("fox"));
    assert_eq!(before.stopped(), StopState::Stopped);
}

#[test]
fn closing_the_find_ends_the_search_and_forgets_its_hits() {
    let mut live = PdfLive::default();
    let stop = live.searching(TypedText::new("fox"));
    live.carry(PdfOut::Find(FindOut::Search(TypedText::new("fox"))));
    assert!(live.wants.search.is_some());
    live.carry(PdfOut::Find(FindOut::Clear));
    assert_eq!(stop.stopped(), StopState::Stopped);
    assert!(live.wants.search.is_none());
    assert!(live.hits().is_empty());
}

#[test]
fn what_the_machine_asks_waits_for_the_view() {
    let mut live = PdfLive::default();
    let place = crate::Destination {
        page: PageIndex(2),
        offset: Permille(250),
    };
    live.carry(PdfOut::ScrollTo(place));
    assert_eq!(live.wants.scroll, Some(place));
    live.carry(PdfOut::Find(FindOut::ShowHit(crate::HitIndex(3))));
    assert_eq!(live.wants.show, Some(crate::HitIndex(3)));
}

#[test]
fn links_and_thumbnails_are_asked_for_once_per_page() {
    let mut live = PdfLive::default();
    assert!(live.links_wanted(PageIndex(0)));
    assert!(!live.links_wanted(PageIndex(0)));
    assert!(live.thumb_wanted(PageIndex(0)));
    assert!(!live.thumb_wanted(PageIndex(0)));
    // A thumbnail that could not be made is asked for again.
    let none = live.received(
        PdfAnswer::Thumb {
            page: PageIndex(0),
            texture: None,
        },
        &Stage::NoStage,
    );
    assert_eq!(none, None::<StageIn>);
    assert!(live.thumb_wanted(PageIndex(0)));
}

#[test]
fn flights_are_numbered_apart() {
    let mut live = PdfLive::default();
    let scene = stack(&[LETTER, LETTER], 1000);
    let asked = live.plan(&scene.schedule(0, 0), PageIndex(0));
    let mut numbers: Vec<FlightId> = asked.iter().map(|asked| asked.flight).collect();
    let all = numbers.len();
    numbers.sort_by_key(|flight| flight.0);
    numbers.dedup();
    assert_eq!(numbers.len(), all);
}
