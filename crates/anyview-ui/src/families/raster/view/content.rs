//! The picture on screen: a clipped room holding one texture layer, and the crop over it.

use super::super::crop::CropOverlay;
use super::super::doc::RasterDoc;
use super::super::geometry::{
    centre_of, fit, frame_of, held_source, in_file, limited_pan, place, point_under, pointer_delta,
    reach_in_picture, scale_of,
};
use super::layout::{CROP_REACH, laid_out, over, rotation, sampling_at, under};
use crate::families::view::{Held, StageCx, WHEEL_ZOOM};
use crate::families::{UnavailableCard, listed, note_of};
use crate::{PictureEditIn, RasterIn, RasterStage, Stage, StageIn};
use anyview_core::{DocPoint, DocUnit, FormatKind, Permille, QuarterTurn, Reflection, Zoom};
use dioxus::prelude::*;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::gesture::{Gesture, WheelDelivery, use_gestures_with};
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};
use ds::prelude::{Point, use_platform};
use ds_blitz::{TexelRect, TextureFit, TextureLayer};
use ds_core::command::holds_primary;

/// The stage: the picture, or the card of a file whose plugin is not installed.
#[component]
pub(crate) fn RasterContent(doc: Held<RasterDoc>, cx: StageCx) -> Element {
    match doc.0.needs.clone() {
        Some(needs) => rsx! { Unshown { doc: doc.clone(), needs, cx: cx.clone() } },
        None => rsx! { PictureContent { doc: doc.clone(), cx: cx.clone() } },
    }
}
/// A file with no picture because the plugin that decodes it is not installed: the card a
/// recording without a player has, with the package that would show it.
#[component]
fn Unshown(doc: Held<RasterDoc>, needs: crate::Need, cx: StageCx) -> Element {
    let note = note_of(Some(&needs), "This picture can\u{2019}t be shown.");
    rsx! {
        UnavailableCard {
            name: doc.0.name.clone(),
            kind: FormatKind::Raster,
            facts: listed(&doc.0.facts),
            note,
            install: needs.helper,
            cx,
        }
    }
}
#[component]
fn PictureContent(doc: Held<RasterDoc>, cx: StageCx) -> Element {
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
                if current.picture.is_dragging() {
                    current.edit.call(PictureEditIn::Drag(under(
                        &current, stage, doc_size, area, at,
                    )));
                    return;
                }
                if let Some(from) = last() {
                    let (kept, turn) = laid_out(&current, stage, doc_size);
                    let shown = scale_of(stage, fit(kept, turn, area));
                    let by = pointer_delta(shown, area, (at.0 - from.0, at.1 - from.1));
                    let by = limited_pan(kept, turn, shown, centre_of(stage, kept, turn), area, by);
                    send.call(StageIn::Raster(RasterIn::PanBy(by)));
                    last.set(Some(at));
                }
            }
            PointerPhase::Release => {
                if current.picture.is_dragging() {
                    current.edit.call(PictureEditIn::Release);
                }
                last.set(None);
                send.call(StageIn::Raster(RasterIn::PanEnd));
            }
            PointerPhase::Press => {}
        }
    });
    let gestured = cx.clone();
    let platform = use_platform();
    // A wheel's detents arrive eased, one share a frame, and a touchpad's run with the glide after it.
    use_gestures_with(WheelDelivery::Eased, move |gesture| {
        let Some(area) = gestured.area else { return };
        let Stage::Raster(stage) = &gestured.stage else {
            return;
        };
        let (kept, turn) = laid_out(&gestured, stage, doc_size);
        let fitted = fit(kept, turn, area);
        let shown = scale_of(stage, fitted);
        let floor = crate::Viewport { shown, fit: fitted }.floor();
        // A pinch and a wheel turn under the primary modifier zoom alike: `by` thousandths, about the pointer,
        // and no farther out than a step out goes.
        let zoom_by = |by: i32, at: Point| {
            let zoom = Zoom::scaled(Permille(
                u32::try_from(i64::from(shown.0) * (1000 + i64::from(by)) / 1000)
                    .unwrap_or(Zoom::MIN_SCALE.0)
                    .max(floor.0),
            ));
            let centre = centre_of(stage, kept, turn);
            let on = point_under(
                centre,
                shown,
                area,
                (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0),
            );
            send.call(StageIn::Raster(RasterIn::SetZoom { zoom, at: on }));
        };
        match gesture {
            Gesture::Pinch { by, at, .. } if over(area, at) => zoom_by(by.0, at),
            Gesture::Scroll { by, at, held, .. }
                if over(area, at) && holds_primary(platform, held) =>
            {
                zoom_by((by.y.0 * WHEEL_ZOOM).round() as i32, at);
            }
            Gesture::Scroll { by, at, .. } if over(area, at) => {
                if let RasterStage::Zoomed { centre, .. } = stage {
                    // The content follows the fingers: the pointer moved by `by`.
                    let moved = pointer_delta(shown, area, (by.x.0, by.y.0));
                    // A share stops at the picture's edge, so a glide ends flush with it (quire
                    // sends it on until it has run out).
                    let moved = limited_pan(kept, turn, shown, *centre, area, moved);
                    if moved
                        == (DocPoint {
                            x: DocUnit(0),
                            y: DocUnit(0),
                        })
                    {
                        return;
                    }
                    send.call(StageIn::Raster(RasterIn::PanStart));
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
    let adjust = cx.picture.adjust();
    let (kept, turn) = laid_out(&cx, stage, doc_size);
    let scale = scale_of(stage, fit(kept, turn, area));
    let centre = centre_of(stage, kept, turn);
    let placed = place(kept, turn, scale, centre, area);
    let origin = adjust.kept(doc_size);
    let zoomed = match stage {
        RasterStage::Fitted { .. } => "fit",
        RasterStage::Zoomed { .. } | RasterStage::Panning { .. } => "zoomed",
    };
    let down = cx.clone();
    let moving = cx.clone();
    let up = cx.clone();
    let overlay = cx.picture.draft().map(|draft| {
        rsx! {
            CropOverlay {
                draft,
                aspect: cx.picture.aspect(),
                dragging: cx.picture.is_dragging(),
                centre,
                scale,
                area,
                onedit: cx.edit,
            }
        }
    });
    rsx! {
        div {
            class: "viewer-raster",
            role: "img",
            "data-zoom": zoomed,
            "data-pan": if cx.hand.pans() { "on" } else { "off" },
            "data-drag": if matches!(stage, RasterStage::Panning { .. }) { "on" } else { "off" },
            "data-crop": if cx.picture.is_cropping() { "on" } else { "off" },
            onmounted: move |event| capture.on_mounted(event),
            onpointerdown: move |event: PointerEvent| {
                let Some(area) = down.area else { return };
                if event.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
                    return;
                }
                let point = event.client_coordinates();
                let at = (
                    point.x as f32 - area.origin.x.0,
                    point.y as f32 - area.origin.y.0,
                );
                if down.picture.is_cropping() {
                    let Stage::Raster(stage) = &down.stage else { return };
                    let (kept, turn) = laid_out(&down, stage, doc_size);
                    let shown = scale_of(stage, fit(kept, turn, area));
                    down.edit.call(PictureEditIn::Grab {
                        at: under(&down, stage, doc_size, area, at),
                        reach: reach_in_picture(shown, area, CROP_REACH),
                    });
                    held.set(capture.begin());
                    return;
                }
                if !down.hand.pans() {
                    return;
                }
                let Stage::Raster(RasterStage::Zoomed { .. }) = &down.stage else { return };
                last.set(Some(at));
                held.set(capture.begin());
                send.call(StageIn::Raster(RasterIn::PanStart));
            },
            onpointermove: move |event: PointerEvent| {
                let Some(area) = moving.area else { return };
                let Stage::Raster(stage) = &moving.stage else { return };
                let point = event.client_coordinates();
                let at = (
                    point.x as f32 - area.origin.x.0,
                    point.y as f32 - area.origin.y.0,
                );
                if moving.picture.is_dragging() {
                    if held() == PointerHold::Local {
                        moving
                            .edit
                            .call(PictureEditIn::Drag(under(&moving, stage, doc_size, area, at)));
                    }
                    return;
                }
                let (Some(from), PointerHold::Local) = (last(), held()) else { return };
                let (kept, turn) = laid_out(&moving, stage, doc_size);
                let shown = scale_of(stage, fit(kept, turn, area));
                let by = pointer_delta(shown, area, (at.0 - from.0, at.1 - from.1));
                let by = limited_pan(kept, turn, shown, centre_of(stage, kept, turn), area, by);
                send.call(StageIn::Raster(RasterIn::PanBy(by)));
                last.set(Some(at));
            },
            onpointerup: move |_| {
                if up.picture.is_dragging() {
                    if held() == PointerHold::Local {
                        up.edit.call(PictureEditIn::Release);
                    }
                    return;
                }
                if held() == PointerHold::Local && last().is_some() {
                    last.set(None);
                    send.call(StageIn::Raster(RasterIn::PanEnd));
                }
            },
            ondoubleclick: move |event: MouseEvent| {
                if cx.picture.is_cropping() {
                    return;
                }
                let Some(area) = cx.area else { return };
                let Stage::Raster(stage) = &cx.stage else { return };
                let point = event.client_coordinates();
                let at = under(
                    &cx,
                    stage,
                    doc_size,
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
                    // The picture is mirrored before it is turned; the transform list reads
                    // right to left.
                    let mirror = match adjust.reflection {
                        Reflection::Kept => "",
                        Reflection::Mirrored => " scaleX(-1)",
                    };
                    let style = format!(
                        "left:{}px; top:{}px; width:{w}px; height:{h}px; transform:rotate({}deg){mirror}",
                        mid_x - w / 2.0,
                        mid_y - h / 2.0,
                        rotation(placed.turn),
                    );
                    let in_the_file = in_file(
                        placed.source,
                        kept,
                        adjust.reflection,
                        (origin.left.0, origin.top.0),
                    );
                    let (x, y, sw, sh) = held_source(in_the_file, doc_size, doc.0.held);
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
            if let Some(overlay) = overlay {
                {overlay}
            }
        }
    }
}
