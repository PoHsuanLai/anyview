//! Tiles drawn through the job runner agree with a whole-page render, and a worker reads a page
//! once per zoom, not once per batch.

use crate::support;

use anyview_core::{Dpi, PageIndex, Permille, PixelLen, PixelSize};
use anyview_pdf::{
    Backend, End, PageLayout, PdfBackend, PdfDocument, PdfDone, PdfJob, PdfWorker, Priority, Stop,
    TILE_SIDE, Ticket, TileBatch, TileCoord, ViewWindow, ZoomBucket, schedule,
};
use std::sync::Arc;
use support::{crop, drawn, fixture, has_ink, pixel, tiles};

fn batch(page: u32, bucket_scale: u32, cells: &[(u32, u32)]) -> TileBatch {
    TileBatch {
        page: PageIndex(page),
        zoom: ZoomBucket::containing(Permille(bucket_scale)),
        priority: Priority::Visible,
        tiles: cells.iter().map(|&(x, y)| TileCoord { x, y }).collect(),
    }
}

fn draw(doc: &PdfDocument, worker: &mut PdfWorker, batch: TileBatch) -> Vec<anyview_pdf::Tile> {
    let job = PdfJob::Tiles {
        ticket: Ticket(1),
        batch,
    };
    let (_, tiles, end) = tiles(PdfBackend::run(doc, worker, job, &Stop::new()));
    assert!(matches!(end, End::Complete), "{end:?}");
    tiles
}

#[test]
fn a_tile_is_the_same_region_cut_from_the_whole_page() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    // Bucket 0 draws a point as a pixel (72 dpi); bucket 4 draws it as two (144 dpi).
    for (scale, dpi) in [(1000, 72), (2000, 144)] {
        let whole = drawn(&doc, 0, Dpi::new(dpi).expect("a resolution"));
        let drawn_tiles = draw(
            &doc,
            &mut worker,
            batch(0, scale, &[(0, 0), (1, 1), (0, 1)]),
        );
        assert_eq!(drawn_tiles.len(), 3, "dpi {dpi}");
        for tile in &drawn_tiles {
            let (x, y) = tile.key.origin();
            let size = tile.raster.size();
            assert!(size.width.0 <= TILE_SIDE && size.height.0 <= TILE_SIDE);
            let want = crop(&whole, x.0, y.0, size.width.0, size.height.0);
            assert_eq!(
                tile.raster.premultiplied_rgba(),
                &want[..],
                "dpi {dpi}, {:?}",
                tile.key
            );
        }
        // A tile that crosses the page's edge is cut to it (page 1 is 612 points across).
        let corner = drawn_tiles.iter().find(|t| (t.key.x, t.key.y) == (1, 1));
        let corner = corner.expect("the corner tile").raster.size();
        let page = whole.size();
        assert_eq!(corner.width.0, (page.width.0 - TILE_SIDE).min(TILE_SIDE));
        assert_eq!(corner.height.0, (page.height.0 - TILE_SIDE).min(TILE_SIDE));
    }
}

#[test]
fn tiles_past_the_real_edge_of_the_page_are_left_out() {
    let doc = fixture();
    let got = draw(
        &doc,
        &mut PdfWorker::new(),
        batch(2, 1000, &[(0, 0), (5, 5)]),
    );
    assert_eq!(got.len(), 1);
    // Page 3 is 400 x 300 points.
    let size = PixelSize {
        width: PixelLen(400),
        height: PixelLen(300),
    };
    assert_eq!(got[0].raster.size(), size);
}

#[test]
fn a_scheduled_view_is_drawn_end_to_end() {
    let doc = fixture();
    let layout = PageLayout::new(doc.page_sizes(), Permille(1000), PixelLen(8));
    let size = PixelSize {
        width: PixelLen(800),
        height: PixelLen(600),
    };
    let window = ViewWindow {
        page: PageIndex(0),
        offset: Permille(0),
        left: -94,
        size,
    };
    let wanted = schedule(&layout, &window, PixelLen(600));
    let mut worker = PdfWorker::new();
    let mut count = 0;
    for job in wanted.batches() {
        let asked = job.tiles.len();
        let got = draw(&doc, &mut worker, job);
        assert!(got.len() <= asked);
        count += got.len();
    }
    assert_eq!(count, wanted.keys().count());
    // The red box on the first page is in the first tile: x 72..272, y 292..392 from the top.
    assert_eq!(wanted.zoom().scale(), Permille(1000));
    let first = &draw(&doc, &mut worker, batch(0, 1000, &[(0, 0)]))[0];
    assert_eq!(pixel(&first.raster, 150, 340), [230, 26, 26, 255]);
    assert!(
        has_ink(&first.raster, 72, 40, 250, 40),
        "the title is drawn"
    );
}

#[test]
fn a_worker_draws_the_next_document_correctly_after_the_first() {
    let (one, two) = (fixture(), fixture());
    let mut worker = PdfWorker::new();
    let mut page = |doc: &PdfDocument| {
        let job = PdfJob::Page {
            ticket: Ticket(1),
            page: PageIndex(1),
            dpi: Dpi::PAGE,
        };
        support::page(PdfBackend::run(doc, &mut worker, job, &Stop::new()))
            .1
            .expect("draws")
    };
    let first = page(&one);
    assert_eq!(first, page(&two));
}

#[test]
fn a_worker_reads_a_page_again_only_for_another_page_zoom_or_document() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    assert_eq!(worker.pages_prepared(), 0);
    let whole = drawn(&doc, 0, Dpi::new(72).expect("a resolution"));
    // Consecutive batches of one page at one zoom share one reading, and what they draw is the
    // whole page cut up, from the held page as from a fresh one.
    for cells in [&[(0, 0)][..], &[(1, 0), (0, 1)], &[(1, 1)]] {
        for tile in draw(&doc, &mut worker, batch(0, 1000, cells)) {
            let (x, y) = tile.key.origin();
            let size = tile.raster.size();
            let want = crop(&whole, x.0, y.0, size.width.0, size.height.0);
            assert_eq!(
                tile.raster.premultiplied_rgba(),
                &want[..],
                "{:?}",
                tile.key
            );
        }
    }
    assert_eq!(worker.pages_prepared(), 1);
    // A new zoom reads the page again, once, however many batches follow.
    draw(&doc, &mut worker, batch(0, 2000, &[(0, 0)]));
    draw(&doc, &mut worker, batch(0, 2000, &[(1, 0)]));
    assert_eq!(worker.pages_prepared(), 2);
    // So does another page, and the page the worker came from, once it has left it.
    draw(&doc, &mut worker, batch(1, 2000, &[(0, 0)]));
    draw(&doc, &mut worker, batch(0, 2000, &[(0, 0)]));
    assert_eq!(worker.pages_prepared(), 4);
    // Another document is another page, though it has the same number.
    let other = fixture();
    draw(&doc, &mut worker, batch(0, 2000, &[(0, 0)]));
    assert_eq!(worker.pages_prepared(), 4);
    draw(&other, &mut worker, batch(0, 2000, &[(0, 0)]));
    assert_eq!(worker.pages_prepared(), 5);
}

#[test]
fn the_document_and_the_worker_cross_threads() {
    fn send<T: Send>() {}
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PdfDocument>();
    send_sync::<Stop>();
    send::<PdfWorker>();
    send::<PdfJob>();
    send::<PdfDone>();
    let doc = Arc::new(fixture());
    let handles: Vec<_> = (0..3)
        .map(|page| {
            let doc = Arc::clone(&doc);
            std::thread::spawn(move || drawn(&doc, page, Dpi::PAGE).size())
        })
        .collect();
    let sizes: Vec<PixelSize> = handles
        .into_iter()
        .map(|h| h.join().expect("a worker"))
        .collect();
    assert_eq!(
        sizes[0],
        PixelSize {
            width: PixelLen(612),
            height: PixelLen(792)
        }
    );
    assert_eq!(
        sizes[2],
        PixelSize {
            width: PixelLen(400),
            height: PixelLen(300)
        }
    );
}
