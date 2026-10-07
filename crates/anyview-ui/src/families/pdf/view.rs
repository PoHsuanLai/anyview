//! The pages on screen: a clipped room holding the pages in view, each a box of tiles, and the
//! pointer and gestures that move it. The view reads the machine's state and sends it inputs; the
//! decisions (which page is at the top, what a zoom step means) are the machine's, and the tiles
//! are asked of the workers here because only the view knows what the room shows.

use super::doc::{PdfDoc, room_of};
use super::draw::{Drawing, UNMEASURED};
use super::find::PdfFinding;
use super::live::{Wants, page_view};
use super::page::PageBox;
use super::scene::{Frame, Scene, fits, scale_of};
use super::steer::{Cursor, Steering};
use super::work::{PdfAsk, PdfTask};
use crate::families::view::{Area, Held, StageCx};
use crate::io::{HostRequest, Job};
use crate::{Destination, FindHits, HitIndex, PageView, PdfIn, PdfStage, Stage, StageIn};
use anyview_core::{PageIndex, Permille, Zoom};
use anyview_pdf::LinkTarget;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::gesture::{Gesture, WheelDelivery, use_gestures_with};
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};
use ds::prelude::Point;
use ds_blitz::use_gpu;
use std::sync::Arc;

/// How far a drag may travel and still be a click on a link, in logical pixels.
const CLICK_SLOP: f32 = 6.0;

/// Thousandths of zoom a logical pixel of a wheel turn under Control makes.
const WHEEL_ZOOM: f32 = 5.0;

/// Where the reader starts when the stage is not the PDF stage yet.
const START: PageView = PageView {
    page: PageIndex(0),
    offset: Permille(0),
    zoom: Zoom::Fit,
};

fn over(area: Area, at: Point) -> bool {
    let (x, y) = (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0);
    x >= 0.0 && y >= 0.0 && x < area.size.width.0 && y < area.size.height.0
}

/// A distance in logical pixels as device pixels.
fn device(logical: f32, area: Area) -> i64 {
    // A pointer distance is far inside i64, so rounding to whole pixels cannot overflow.
    (logical * area.scale).round() as i64
}

/// `logical` and the `carried` remainder as whole device pixels, and the remainder left over.
fn whole_pixels(logical: f32, carried: f32, area: Area) -> (i64, f32) {
    let total = logical * area.scale + carried;
    let whole = total.round();
    (whole as i64, total - whole)
}

/// A point of the window as a point of the room, in device pixels.
fn in_room(at: Point, area: Area) -> (u32, u32) {
    let across = |value: f32, origin: f32| u32::try_from(device(value - origin, area)).unwrap_or(0);
    (
        across(at.x.0, area.origin.x.0),
        across(at.y.0, area.origin.y.0),
    )
}

/// The hit the machine is on, when a find has one.
fn current_hit(stage: &Stage) -> Option<HitIndex> {
    match stage {
        Stage::Pdf(PdfStage::Finding { hits, .. }) => match hits {
            FindHits::Found(_) => hits.current(),
            FindHits::Idle | FindHits::Pending | FindHits::NoMatch => None,
        },
        Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Media(_)
        | Stage::Book(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}

/// The web address a link may open: only the schemes a person expects a PDF to open.
fn web_address(uri: &str) -> Option<String> {
    let (scheme, _) = uri.split_once(':')?;
    const SCHEMES: [&str; 3] = ["http", "https", "mailto"];
    SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
        .then(|| uri.to_owned())
}

#[component]
pub(super) fn PdfContent(doc: Held<PdfDoc>, cx: StageCx) -> Element {
    let gpu = use_gpu();
    let area = room_of(cx.area);
    let frame = area.map_or(UNMEASURED, Frame::of);
    let view = page_view(&cx.stage).unwrap_or(START);
    let fit_doc = Arc::clone(&doc.0);
    let fit = use_memo(use_reactive!(|frame| fits(fit_doc.sizes(), frame)));
    let shown = scale_of(view.zoom, fit());
    let scene_doc = Arc::clone(&doc.0);
    let scene = use_memo(use_reactive!(|frame, shown| Scene::new(
        scene_doc.sizes(),
        frame,
        shown
    )));
    let mut cursor = use_signal(|| Cursor {
        top: 0,
        pan: 0,
        scale: shown,
    });
    let steer = Steering::new(cursor, scene, cx.send);
    let live = cx.pdf;

    // Where the room is: the cursor while it agrees with the machine, the machine's otherwise.
    let here = cursor();
    let agrees = here.scale == shown && scene().place_at(here.top) == (view.page, view.offset);
    let top = if agrees {
        here.top
    } else {
        scene().top_of(view.page, view.offset)
    };
    let pan = here.pan.min(scene().widest());

    // A change the machine made that the cursor did not (a jump, a zoom step, a resize) moves the
    // cursor to the machine's place.
    use_effect(use_reactive!(|view, shown, frame| {
        let (scene, now) = (scene.peek(), *cursor.peek());
        let held = (view.page, view.offset);
        if now.scale != shown || scene.place_at(now.top) != held {
            cursor.set(Cursor {
                top: scene.top_of(view.page, view.offset),
                pan: now.pan.min(scene.widest()),
                scale: shown,
            });
        }
        let _ = frame;
    }));

    let ready = gpu.device().is_some();
    let (work, ticket, request) = (cx.work, cx.ticket, cx.request);
    let tiles_doc = Arc::clone(&doc.0);
    let tiles_gpu = gpu.clone();
    // The tiles the room needs, and the links of the pages in it.
    use_effect(use_reactive!(|top, pan, shown, frame, ready| {
        if !ready {
            return;
        }
        let scene = scene.peek();
        let hand = |ask: PdfAsk| {
            let task = PdfTask::new(ticket, Arc::clone(&tiles_doc), tiles_gpu.clone(), ask);
            work.call(Job::Pdf(task));
        };
        let reader = scene.place_at(top).0;
        let asked = live.with_mut(|live| live.plan(&scene.schedule(top, pan), reader));
        for asked in asked {
            hand(PdfAsk::Tiles {
                flight: asked.flight,
                batch: asked.batch,
                stop: asked.stop,
            });
        }
        let view = scene.view(top, pan);
        let (from, to) = (
            u64::try_from(view.top).unwrap_or(0),
            u64::try_from(view.bottom).unwrap_or(0),
        );
        for (page, _) in scene.layout.between(from, to) {
            if live.with_mut(|live| live.links_wanted(page)) {
                hand(PdfAsk::Links { page });
            }
        }
        let _ = (shown, frame);
    }));

    // What the machine's outputs asked: a search to run, a hit to show, a place to go to.
    let wants = live.read().wants.clone();
    let wants_doc = Arc::clone(&doc.0);
    let wants_gpu = gpu.clone();
    use_effect(use_reactive!(|wants| {
        if wants == Wants::default() {
            return;
        }
        if let Some(place) = wants.scroll {
            steer.go_to(place);
        }
        // Copied out: showing the hit moves the machine, whose outputs write the shelf.
        let found = wants
            .show
            .and_then(|hit| live.peek().hits().get(hit.0).cloned());
        if let Some(found) = found {
            steer.show(found.page, &found.rects);
        }
        if let Some(query) = wants.search.clone() {
            let stop = live.with_mut(|live| live.searching(query.clone()));
            let ask = PdfAsk::Search { query, stop };
            let task = PdfTask::new(ticket, Arc::clone(&wants_doc), wants_gpu.clone(), ask);
            work.call(Job::Pdf(task));
        }
        live.with_mut(|live| live.wants = Wants::default());
    }));

    // A wheel's detents arrive eased, one share a frame, each a few device pixels; the part of a
    // share below a whole pixel is carried to the next, so the shares still sum to the detent.
    let mut carried = use_signal(|| (0.0_f32, 0.0_f32));
    use_gestures_with(WheelDelivery::Eased, move |gesture| {
        let Some(area) = area else { return };
        match gesture {
            Gesture::Pinch { by, at, .. } if over(area, at) => {
                steer.zoom_by(by.0, in_room(at, area));
            }
            Gesture::Scroll { by, at, held, .. } if over(area, at) => {
                if held.intersects(Modifiers::CONTROL | Modifiers::META) {
                    let turn = (by.y.0 * WHEEL_ZOOM).round() as i32;
                    steer.zoom_by(turn, in_room(at, area));
                } else {
                    let (dx, left_x) = whole_pixels(by.x.0, carried.peek().0, area);
                    let (dy, left_y) = whole_pixels(by.y.0, carried.peek().1, area);
                    carried.set((left_x, left_y));
                    steer.scroll_by(dx, dy);
                }
            }
            Gesture::Pinch { .. } | Gesture::Scroll { .. } => {}
        }
    });

    let mut last = use_signal(|| None::<(f32, f32)>);
    let mut travelled = use_signal(|| 0.0_f32);
    let mut held = use_signal(|| PointerHold::Local);
    let capture = use_pointer_capture(move |pointer: CapturedPointer| match pointer.phase {
        PointerPhase::Drag => {
            if let (Some(from), Some(area)) = (last(), area) {
                let at = (pointer.at.x.0, pointer.at.y.0);
                steer.scroll_by(device(at.0 - from.0, area), device(at.1 - from.1, area));
                travelled.with_mut(|far| *far += (at.0 - from.0).abs() + (at.1 - from.1).abs());
                last.set(Some(at));
            }
        }
        PointerPhase::Release => last.set(None),
        PointerPhase::Press => {}
    });

    let onlink = EventHandler::new(move |target: LinkTarget| {
        if travelled() > CLICK_SLOP {
            return;
        }
        match target {
            LinkTarget::Page(page) => cx.send.call(StageIn::Pdf(PdfIn::GoTo(Destination {
                page,
                offset: Permille(0),
            }))),
            LinkTarget::Uri(uri) => {
                if let Some(address) = web_address(&uri) {
                    request.call(HostRequest::OpenUri(address));
                }
            }
            LinkTarget::Other => {}
        }
    });

    let Some(area) = area else {
        return rsx! { div { class: "viewer-pdf" } };
    };
    let pages = {
        let scene = scene();
        let live = live.read();
        Drawing {
            scene: &scene,
            live: &live,
            top,
            pan,
            current: current_hit(&cx.stage),
        }
        .pages()
    };
    let moving = area;
    rsx! {
        div {
            class: "viewer-pdf",
            role: "document",
            onmounted: move |event| capture.on_mounted(event),
            onpointerdown: move |event: PointerEvent| {
                if event.trigger_button() != Some(MouseButton::Primary) {
                    return;
                }
                let at = event.client_coordinates();
                last.set(Some((at.x as f32, at.y as f32)));
                travelled.set(0.0);
                held.set(capture.begin());
            },
            onpointermove: move |event: PointerEvent| {
                let (Some(from), PointerHold::Local) = (last(), held()) else { return };
                let at = event.client_coordinates();
                let at = (at.x as f32, at.y as f32);
                steer.scroll_by(device(at.0 - from.0, moving), device(at.1 - from.1, moving));
                travelled.with_mut(|far| *far += (at.0 - from.0).abs() + (at.1 - from.1).abs());
                last.set(Some(at));
            },
            onpointerup: move |_| last.set(None),
            for page in pages {
                PageBox {
                    key: "{page.number}",
                    number: page.number,
                    style: page.style,
                    tiles: page.tiles,
                    marks: page.marks,
                    links: page.links,
                    onlink,
                }
            }
        }
        PdfFinding { cx: cx.clone() }
    }
}
