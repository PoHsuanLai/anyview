//! Stale tickets and stops: a result says which request it answers, a raised stop or a passed
//! deadline ends a job, and neither harms the document or the worker.

mod support;

use anyview_core::{Dpi, PageIndex, PageSelection, Permille};
use anyview_pdf::{
    Backend, End, PdfBackend, PdfDocument, PdfError, PdfJob, PdfWorker, Priority, SearchQuery,
    Stop, Ticket, TileBatch, TileCoord, ZoomBucket,
};
use std::thread;
use std::time::{Duration, Instant};
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

/// A one-page file of six hundred thousand tiny rectangles: a tile takes long enough to draw that a
/// stop raised from another thread lands between its objects.
fn heavy() -> PdfDocument {
    let rectangles = "0.3 0.5 0.7 rg 10 10 3 3 re f\n".repeat(600_000);
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /ExtGState << /G << /ca 0.5 >> >> >> >>"
            .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{rectangles}endstream",
            rectangles.len()
        ),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).expect("the heavy file opens")
}

#[test]
fn a_flag_raised_from_another_thread_ends_a_draw_in_the_middle_of_a_tile() {
    let doc = heavy();
    let mut worker = PdfWorker::new();
    let job = |ticket, cells| PdfJob::Tiles {
        ticket: Ticket(ticket),
        batch: batch(cells),
    };
    // A first, uninterrupted batch reads the page at this zoom; a second one, timed, is all
    // drawing.
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job(1, 1), &Stop::new()));
    assert!(drawn.len() == 1 && matches!(end, End::Complete), "{end:?}");
    let started = Instant::now();
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job(2, 1), &Stop::new()));
    let one_tile = started.elapsed();
    assert!(drawn.len() == 1 && matches!(end, End::Complete), "{end:?}");

    let stop = Stop::new();
    let raiser = stop.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(one_tile / 10);
        raiser.request();
    });
    let started = Instant::now();
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job(3, 1), &stop));
    let cut = started.elapsed();
    canceller.join().expect("the canceller finishes");
    assert!(drawn.is_empty(), "{} tiles", drawn.len());
    assert!(matches!(end, End::Stopped), "{end:?}");
    assert!(cut < one_tile, "{cut:?} is not before {one_tile:?}");

    // The flag is spent; a fresh stop draws on the same worker, from the page it still holds.
    let prepared = worker.pages_prepared();
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job(4, 1), &Stop::new()));
    assert!(drawn.len() == 1 && matches!(end, End::Complete), "{end:?}");
    assert_eq!(worker.pages_prepared(), prepared);
}

#[test]
fn a_deadline_ends_a_draw_in_the_middle_of_a_tile_as_stopped() {
    let doc = heavy();
    let mut worker = PdfWorker::new();
    let job = |ticket| PdfJob::Tiles {
        ticket: Ticket(ticket),
        batch: batch(1),
    };
    let _ = PdfBackend::run(&doc, &mut worker, job(1), &Stop::new());
    let soon = Stop::with_deadline(Instant::now() + Duration::from_millis(10));
    let (_, drawn, end) = tiles(PdfBackend::run(&doc, &mut worker, job(2), &soon));
    assert!(drawn.is_empty() && matches!(end, End::Stopped), "{end:?}");
}
