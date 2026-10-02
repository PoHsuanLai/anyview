//! Stale tickets and stops: a result says which request it answers, a raised stop or a passed
//! deadline ends a job, and neither harms the document or the worker.

mod support;

use anyview_core::{Dpi, PageIndex, PageSelection, Permille};
use anyview_pdf::{
    Backend, End, PdfBackend, PdfError, PdfJob, PdfWorker, Priority, SearchQuery, Stop, Ticket,
    TileBatch, TileCoord, ZoomBucket,
};
use std::time::Instant;
use support::{fixture, page, searched, tiles, written};

fn batch(cells: u32) -> TileBatch {
    TileBatch {
        page: PageIndex(0),
        zoom: ZoomBucket::containing(Permille(1000)),
        priority: Priority::Visible,
        tiles: (0..cells).map(|x| TileCoord { x, y: 0 }).collect(),
    }
}

fn stopped() -> Stop {
    let stop = Stop::new();
    stop.request();
    stop
}

#[test]
fn a_result_carries_the_ticket_of_its_job_so_a_late_one_can_be_dropped() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    let mut latest = Ticket::default();
    let mut results = Vec::new();
    // Two requests; the person moves on after the first, so only the second is wanted.
    for _ in 0..2 {
        latest = latest.next();
        let job = PdfJob::Tiles {
            ticket: latest,
            batch: batch(2),
        };
        results.push(tiles(PdfBackend::run(&doc, &mut worker, job, &Stop::new())));
    }
    let kept: Vec<Ticket> = results
        .iter()
        .map(|r| r.0)
        .filter(|t| *t == latest)
        .collect();
    assert_eq!(kept, [Ticket(2)]);
    assert_eq!(
        results.iter().map(|r| r.0).collect::<Vec<_>>(),
        [Ticket(1), Ticket(2)]
    );
}

#[test]
fn a_raised_stop_ends_a_batch_before_it_draws_anything() {
    let doc = fixture();
    let stop = Stop::new();
    let held_by_whoever_superseded_the_job = stop.clone();
    held_by_whoever_superseded_the_job.request();
    let job = PdfJob::Tiles {
        ticket: Ticket(7),
        batch: batch(2),
    };
    let (ticket, drawn, end) = tiles(PdfBackend::run(&doc, &mut PdfWorker::new(), job, &stop));
    assert_eq!(ticket, Ticket(7));
    assert!(drawn.is_empty());
    assert!(matches!(end, End::Stopped), "{end:?}");
}

#[test]
fn a_passed_deadline_stops_a_job_the_same_way() {
    let doc = fixture();
    let spent = Stop::with_deadline(Instant::now());
    let mut worker = PdfWorker::new();
    let job = PdfJob::Page {
        ticket: Ticket(1),
        page: PageIndex(0),
        dpi: Dpi::SCREEN,
    };
    let (_, raster) = page(PdfBackend::run(&doc, &mut worker, job, &spent));
    assert!(matches!(raster, Err(PdfError::Stopped)), "{raster:?}");
    let job = PdfJob::Tiles {
        ticket: Ticket(2),
        batch: batch(2),
    };
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job, &spent));
    assert!(drawn.is_empty() && matches!(end, End::Stopped));
}

#[test]
fn after_a_stop_the_document_and_the_worker_draw_again() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    let job = PdfJob::Tiles {
        ticket: Ticket(1),
        batch: batch(1),
    };
    let _ = PdfBackend::run(&doc, &mut worker, job, &stopped());
    // A fresh stop per job: the worker's session no longer holds the stopped one.
    let job = PdfJob::Tiles {
        ticket: Ticket(2),
        batch: batch(2),
    };
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job, &Stop::new()));
    assert_eq!(drawn.len(), 2);
    assert!(matches!(end, End::Complete));
}

#[test]
fn a_stopped_search_finds_nothing_new_and_a_free_one_finds_everything() {
    let doc = fixture();
    let mut worker = PdfWorker::new();
    let job = PdfJob::Search {
        ticket: Ticket(3),
        query: SearchQuery::new("fox"),
    };
    let (_, hits, end) = searched(PdfBackend::run(&doc, &mut worker, job.clone(), &stopped()));
    assert!(hits.is_empty() && matches!(end, End::Stopped));
    let (ticket, hits, end) = searched(PdfBackend::run(&doc, &mut worker, job, &Stop::new()));
    assert_eq!((ticket, hits.len()), (Ticket(3), 3));
    assert!(matches!(end, End::Complete));
}

#[test]
fn work_that_cannot_be_interrupted_is_not_started_once_stopped() {
    let doc = fixture();
    let job = PdfJob::WritePdf {
        ticket: Ticket(1),
        pages: PageSelection::All,
    };
    let file = written(PdfBackend::run(
        &doc,
        &mut PdfWorker::new(),
        job,
        &stopped(),
    ));
    assert!(matches!(file, Err(PdfError::Stopped)));
}
