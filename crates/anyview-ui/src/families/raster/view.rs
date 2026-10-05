//! The picture on screen: a clipped room holding one `TextureLayer` at the box geometry placed,
//! and the pointer and gestures that move it. The view reads the machine's state and sends it
//! inputs; every decision (what a pinch means at this zoom, where a double-click goes) is the
//! machine's.

use super::doc::RasterDoc;
use super::geometry::{
    centre_of, fit, frame_of, held_source, place, point_under, pointer_delta, scale_of, turn_of,
};
use crate::families::view::{Area, Held, StageCx};
use crate::{RasterIn, RasterStage, Stage, StageIn};
use anyview_core::{Permille, QuarterTurn, Zoom};
use dioxus::prelude::*;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::gesture::{Gesture, use_gestures};
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};
use ds::prelude::Point;
use ds_blitz::{Sampling, TexelRect, TextureFit, TextureLayer};

/// Whether `at`, a point of the window, is over the room.
fn over(area: Area, at: Point) -> bool {
    let (x, y) = (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0);
    x >= 0.0 && y >= 0.0 && x < area.size.width.0 && y < area.size.height.0
}

/// How the texture is resampled at `scale`: smooth when shrunk, hard pixels when blown up.
fn sampling_at(scale: Permille) -> Sampling {
    match scale.0 {
        0..1000 => Sampling::Bicubic,
        1000..3000 => Sampling::Bilinear,
        _ => Sampling::Nearest,
    }
}

/// The CSS rotation of the texture's box.
fn rotation(turn: QuarterTurn) -> u16 {
    turn.degrees()
}

#[component]
pub(super) fn RasterContent(doc: Held<RasterDoc>, cx: StageCx) -> Element {
    let mut last = use_signal(|| None::<(f32, f32)>);
    let mut held = use_signal(|| PointerHold::Local);
    let send = cx.send;
    let current = cx.clone();
    let doc_size = doc.0.size;
    let capture = use_pointer_capture(move |pointer: CapturedPointer| {
        let Some(area) = current.area else { return };
        let Stage::Raster(stage) = &current.stage else {
            return;
        };
        let at = (
            pointer.at.x.0 - area.origin.x.0,
            pointer.at.y.0 - area.origin.y.0,
        );
        match pointer.phase {
            PointerPhase::Drag => {
                if let Some(from) = last() {
                    let shown = scale_of(stage, fit(doc_size, turn_of(stage), area));
                    let by = pointer_delta(shown, area, (at.0 - from.0, at.1 - from.1));
                    send.call(StageIn::Raster(RasterIn::PanBy(by)));
                    last.set(Some(at));
                }
            }
            PointerPhase::Release => {
                last.set(None);
                send.call(StageIn::Raster(RasterIn::PanEnd));
            }
            PointerPhase::Press => {}
        }
    });
    let gestured = cx.clone();
    use_gestures(move |gesture| {
        let Some(area) = gestured.area else { return };
        let Stage::Raster(stage) = &gestured.stage else {
            return;
        };
        let shown = scale_of(stage, fit(doc_size, turn_of(stage), area));
        match gesture {
            Gesture::Pinch { by, at, .. } if over(area, at) => {
                let zoom = Zoom::scaled(Permille(
                    u32::try_from(i64::from(shown.0) * (1000 + i64::from(by.0)) / 1000)
                        .unwrap_or(Zoom::MIN_SCALE.0),
                ));
                let centre = centre_of(stage, doc_size);
                let on = point_under(
                    centre,
                    shown,
                    area,
                    (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0),
                );
                send.call(StageIn::Raster(RasterIn::SetZoom { zoom, at: on }));
            }
            Gesture::Scroll { by, at, .. } if over(area, at) => {
                if let RasterStage::Zoomed { .. } = stage {
                    send.call(StageIn::Raster(RasterIn::PanStart));
                    // The content follows the fingers: the pointer moved by `by`.
                    let moved = pointer_delta(shown, area, (by.x.0, by.y.0));
                    send.call(StageIn::Raster(RasterIn::PanBy(moved)));
                    send.call(StageIn::Raster(RasterIn::PanEnd));
                }
            }
            Gesture::Pinch { .. } | Gesture::Scroll { .. } => {}
        }
    });

    let (Some(area), Stage::Raster(stage)) = (cx.area, &cx.stage) else {
        return rsx! { div { class: "viewer-raster" } };
    };
    let turn = turn_of(stage);
    let scale = scale_of(stage, fit(doc_size, turn, area));
    let centre = centre_of(stage, doc_size);
    let placed = place(doc_size, turn, scale, centre, area);
    let zoomed = match stage {
        RasterStage::Fitted { .. } => "fit",
        RasterStage::Zoomed { .. } | RasterStage::Panning { .. } => "zoomed",
    };
    let down = cx.clone();
    let moving = cx.clone();
    rsx! {
        div {
            class: "viewer-raster",
            role: "img",
            "data-zoom": zoomed,
            onmounted: move |event| capture.on_mounted(event),
            onpointerdown: move |event: PointerEvent| {
                let Some(area) = down.area else { return };
                let Stage::Raster(RasterStage::Zoomed { .. }) = &down.stage else { return };
                if event.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
                    return;
                }
                let point = event.client_coordinates();
                last.set(Some((
                    point.x as f32 - area.origin.x.0,
                    point.y as f32 - area.origin.y.0,
                )));
                held.set(capture.begin());
                send.call(StageIn::Raster(RasterIn::PanStart));
            },
            onpointermove: move |event: PointerEvent| {
                let (Some(from), PointerHold::Local) = (last(), held()) else { return };
                let Some(area) = moving.area else { return };
                let Stage::Raster(stage) = &moving.stage else { return };
                let point = event.client_coordinates();
                let at = (
                    point.x as f32 - area.origin.x.0,
                    point.y as f32 - area.origin.y.0,
                );
                let shown = scale_of(stage, fit(doc_size, turn_of(stage), area));
                let by = pointer_delta(shown, area, (at.0 - from.0, at.1 - from.1));
                send.call(StageIn::Raster(RasterIn::PanBy(by)));
                last.set(Some(at));
            },
            onpointerup: move |_| {
                if held() == PointerHold::Local && last().is_some() {
                    last.set(None);
                    send.call(StageIn::Raster(RasterIn::PanEnd));
                }
            },
            ondoubleclick: move |event: MouseEvent| {
                let Some(area) = cx.area else { return };
                let Stage::Raster(stage) = &cx.stage else { return };
                let point = event.client_coordinates();
                let shown = scale_of(stage, fit(doc_size, turn_of(stage), area));
                let at = point_under(
                    centre_of(stage, doc_size),
                    shown,
                    area,
                    (
                        point.x as f32 - area.origin.x.0,
                        point.y as f32 - area.origin.y.0,
                    ),
                );
                send.call(StageIn::Raster(RasterIn::DoubleClick { at }));
            },
            if let Some(placed) = placed {
                {
                    let (left, top, width, height) = placed.shown;
                    let (mid_x, mid_y) = (left + width / 2.0, top + height / 2.0);
                    let (w, h) = match placed.turn {
                        QuarterTurn::None | QuarterTurn::Half => (width, height),
                        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => (height, width),
                    };
                    let style = format!(
                        "left:{}px; top:{}px; width:{w}px; height:{h}px; transform:rotate({}deg)",
                        mid_x - w / 2.0,
                        mid_y - h / 2.0,
                        rotation(placed.turn),
                    );
                    let (x, y, sw, sh) = held_source(placed.source, doc_size, doc.0.held);
                    let texture = doc.0.texture_at(frame_of(stage)).clone();
                    rsx! {
                        div { class: "viewer-raster-picture", style,
                            TextureLayer {
                                texture: Some(texture),
                                fit: TextureFit::Fill,
                                source: Some(TexelRect::new(x, y, sw, sh)),
                                sampling: sampling_at(scale),
                            }
                        }
                    }
                }
            }
        }
    }
}
