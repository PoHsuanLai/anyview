//! The pages on screen: a clipped room holding the pages in view, each a box of tiles, and the
//! pointer and gestures that move it. The view reads the machine's state and sends it inputs; the
//! decisions (which page is at the top, what a zoom step means) are the machine's, and the tiles
//! are asked of the workers here because only the view knows what the room shows.

use super::doc::{PdfDoc, PdfOrigin, room_of};
use super::dock::MarkupDock;
use super::draw::{Drawing, UNMEASURED};
use super::live::{Wants, page_view};
use super::page::PageBox;
use super::scene::{Frame, Scene, fits, scale_of};
use super::steer::{Cursor, Steering};
use super::words;
use super::work::{PdfAsk, PdfTask};
use crate::families::view::{Area, Held, StageCx, WHEEL_ZOOM};
use crate::io::{HostRequest, Job};
use crate::{
    Destination, FindHits, HitIndex, MarkupIn, PageView, PdfIn, PdfStage, Stage, StageIn, Tool,
};
use anyview_core::{PageIndex, Permille, Zoom};
use anyview_pdf::LinkTarget;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::gesture::{Gesture, WheelDelivery, use_gestures_with};
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};
use ds::prelude::{Point, use_platform};
use ds_blitz::use_gpu;
use ds_core::command::holds_primary;
use ds_core::word::Word;
use std::sync::Arc;

/// How near a mark's ink a press counts as on it, in thousandths of the page.
const REACH: u32 = 8;

/// What a drag with the primary button does on the pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
enum PointerUse {
    /// It moves the pages.
    Panning,
    /// It selects the words under it.
    Selecting,
    /// It draws, picks and moves marks.
    Marking,
}

/// A drag that is not a pan: the page it went down on, and where the words it selects begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dragging {
    /// Words being selected from the one at `anchor`.
    Words { page: PageIndex, anchor: usize },
    /// A shape being drawn, or a mark being moved.
    Shape { page: PageIndex },
}

/// A point of the window as a point of the room, in device pixels; a point above or left of the
/// room is its edge.
fn room_device(at: (f32, f32), area: Area) -> (u32, u32) {
    let across = |value: f32, origin: f32| u32::try_from(device(value - origin, area)).unwrap_or(0);
    (across(at.0, area.origin.x.0), across(at.1, area.origin.y.0))
}

/// How far a drag may travel and still be a click on a link, in logical pixels.
const CLICK_SLOP: f32 = 6.0;

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
    room_device((at.x.0, at.y.0), area)
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
    let mark = cx.mark;
    // What a drag does follows the pointer tool; a book bound as a PDF has pages to read only.
    let pointer = match (doc.0.origin, cx.hand.tool) {
        (PdfOrigin::File, Tool::Select) => PointerUse::Selecting,
        (PdfOrigin::File, Tool::Markup) if cx.markup.can_mark() => PointerUse::Marking,
        (PdfOrigin::File, Tool::Markup | Tool::Pan | Tool::Crop)
        | (
            PdfOrigin::Book | PdfOrigin::Comic,
            Tool::Select | Tool::Pan | Tool::Crop | Tool::Markup,
        ) => PointerUse::Panning,
    };
    let words_tool = cx.markup.tool().marks_words();
    let tool = cx.markup.tool();
    let picked = cx.markup.picked();
    let writable = picked.is_some_and(|index| cx.markup.words_of(index).is_some());
    let mut dragging = use_signal(|| None::<Dragging>);

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
    let selecting = pointer != PointerUse::Panning;
    use_effect(use_reactive!(|top, pan, shown, frame, ready, selecting| {
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
            if selecting && live.with_mut(|live| live.words_wanted(page)) {
                hand(PdfAsk::Words { page });
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
        if let Some((query, options)) = wants.search.clone() {
            let stop = live.with_mut(|live| live.searching(query.clone(), options));
            let ask = PdfAsk::Search {
                query,
                options,
                stop,
            };
            let task = PdfTask::new(ticket, Arc::clone(&wants_doc), wants_gpu.clone(), ask);
            work.call(Job::Pdf(task));
        }
        live.with_mut(|live| live.wants = Wants::default());
    }));

    // A wheel's detents arrive eased, one share a frame, each a few device pixels; the part of a
    // share below a whole pixel is carried to the next, so the shares still sum to the detent.
    let mut carried = use_signal(|| (0.0_f32, 0.0_f32));
    let platform = use_platform();
    use_gestures_with(WheelDelivery::Eased, move |gesture| {
        let Some(area) = area else { return };
        match gesture {
            Gesture::Pinch { by, at, .. } if over(area, at) => {
                steer.zoom_by(by.0, in_room(at, area));
            }
            Gesture::Scroll { by, at, held, .. } if over(area, at) => {
                if holds_primary(platform, held) {
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
    // A press that selects or draws begins on the page under it. The words of the page have to be
    // read first to select them.
    let begin = move |at: (f32, f32)| -> Option<Dragging> {
        let area = area?;
        let (page, point) = scene.peek().page_point(top, pan, room_device(at, area))?;
        if pointer == PointerUse::Selecting || (pointer == PointerUse::Marking && words_tool) {
            let read = live.peek().words(page)?;
            let anchor = words::nearest(&read, point)?;
            mark.call(MarkupIn::Select(None));
            Some(Dragging::Words { page, anchor })
        } else {
            mark.call(MarkupIn::Press {
                page,
                at: point,
                reach: REACH,
            });
            Some(Dragging::Shape { page })
        }
    };
    // The pointer moved with the button down: the words from the first to the one under it are
    // selected, or the shape grows to the point on the page it went down on.
    let drive = move |drag: Dragging, at: (f32, f32)| {
        let Some(area) = area else { return };
        let device = room_device(at, area);
        match drag {
            Dragging::Words { page, anchor } => {
                let point = scene.peek().point_on(page, top, pan, device);
                let Some(read) = live.peek().words(page) else {
                    return;
                };
                if let Some(head) = words::nearest(&read, point) {
                    mark.call(MarkupIn::Select(Some((
                        page,
                        words::between(&read, anchor, head),
                    ))));
                }
            }
            Dragging::Shape { page } => {
                mark.call(MarkupIn::Drag(
                    scene.peek().point_on(page, top, pan, device),
                ));
            }
        }
    };
    let capture = use_pointer_capture(move |pointer_at: CapturedPointer| match pointer_at.phase {
        PointerPhase::Drag => {
            let at = (pointer_at.at.x.0, pointer_at.at.y.0);
            if let Some(drag) = dragging() {
                drive(drag, at);
            } else if let (Some(from), Some(area)) = (last(), area) {
                steer.scroll_by(device(at.0 - from.0, area), device(at.1 - from.1, area));
                travelled.with_mut(|far| *far += (at.0 - from.0).abs() + (at.1 - from.1).abs());
                last.set(Some(at));
            }
        }
        PointerPhase::Release => {
            if let Some(drag) = dragging() {
                released(drag, pointer, tool, mark);
                dragging.set(None);
            }
            last.set(None);
        }
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
            markup: &cx.markup,
        }
        .pages()
    };
    let (marking_color, any_picked) = (cx.markup.color(), picked.is_some());
    let run = cx.run;
    let moving = area;
    rsx! {
        div {
            class: "viewer-pdf",
            role: "document",
            "data-pointer": pointer.slug(),
            onmounted: move |event| capture.on_mounted(event),
            onpointerdown: move |event: PointerEvent| {
                if event.trigger_button() != Some(MouseButton::Primary) {
                    return;
                }
                let at = event.client_coordinates();
                let at = (at.x as f32, at.y as f32);
                if pointer == PointerUse::Panning {
                    last.set(Some(at));
                    travelled.set(0.0);
                    held.set(capture.begin());
                } else if let Some(drag) = begin(at) {
                    dragging.set(Some(drag));
                    held.set(capture.begin());
                }
            },
            onpointermove: move |event: PointerEvent| {
                let at = event.client_coordinates();
                let at = (at.x as f32, at.y as f32);
                if let Some(drag) = dragging() {
                    if matches!(held(), PointerHold::Local) {
                        drive(drag, at);
                    }
                    return;
                }
                let (Some(from), PointerHold::Local) = (last(), held()) else { return };
                steer.scroll_by(device(at.0 - from.0, moving), device(at.1 - from.1, moving));
                travelled.with_mut(|far| *far += (at.0 - from.0).abs() + (at.1 - from.1).abs());
                last.set(Some(at));
            },
            onpointerup: move |_| {
                if let Some(drag) = dragging() {
                    released(drag, pointer, tool, mark);
                    dragging.set(None);
                }
                last.set(None);
            },
            ondoubleclick: move |_| {
                if pointer == PointerUse::Marking
                    && writable
                    && let Some(index) = picked
                {
                    mark.call(MarkupIn::Write(index));
                }
            },
            for page in pages {
                PageBox {
                    key: "{page.number}",
                    number: page.number,
                    style: page.style,
                    tiles: page.tiles,
                    marks: page.marks,
                    links: page.links,
                    onlink,
                    fields: page.fields,
                    inks: page.inks,
                    writing: page.writing,
                    onmark: mark,
                }
            }
            if pointer == PointerUse::Marking {
                MarkupDock { tool, color: marking_color, picked: any_picked, run }
            }
        }
    }
}

/// The pointer was let go after a drag that selects or draws: words drawn over with the tool that
/// marks words are marked, and a shape is made.
fn released(
    drag: Dragging,
    pointer: PointerUse,
    tool: anyview_core::MarkTool,
    mark: EventHandler<MarkupIn>,
) {
    match drag {
        Dragging::Words { .. } => {
            if pointer == PointerUse::Marking {
                mark.call(MarkupIn::Apply(tool));
            }
        }
        Dragging::Shape { .. } => mark.call(MarkupIn::Release),
    }
}
