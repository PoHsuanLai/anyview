//! The picture on screen: a clipped room holding one `TextureLayer` at the box geometry placed,
//! and the pointer and gestures that move it. The view reads the machine's state and sends it
//! inputs; every decision (what a pinch means at this zoom, where a double-click goes) is the
//! machine's.

use super::crop::CropOverlay;
use super::doc::RasterDoc;
use super::geometry::{
    centre_of, fit, frame_of, held_source, in_file, limited_pan, place, point_under, pointer_delta,
    reach_in_picture, scale_of, turn_of,
};
use crate::families::view::{Area, Held, StageCx, WHEEL_ZOOM};
use crate::{Command, PictureEditIn, RasterIn, RasterStage, Stage, StageIn, Tool};
use anyview_core::{DocPoint, DocUnit, Permille, PixelSize, QuarterTurn, Reflection, Zoom};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::button::Button;
use ds::components::controls::segmented::Tracking;
use ds::components::fields::fact_list::FactList;
use ds::components::overlays::empty_state::EmptyState;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::gesture::{Gesture, WheelDelivery, use_gestures_with};
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};
use ds::prelude::{Choice, Icon, SegmentedControl, Shortcut, ShortcutKey, Tooltip};
use ds::prelude::{Point, use_platform};
use ds_blitz::{Sampling, TexelRect, TextureFit, TextureLayer};
use ds_core::command::holds_primary;
use ds_core::word::Word;

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

/// The stage: the picture, or the card of a file whose plugin is not installed.
#[component]
pub(super) fn RasterContent(doc: Held<RasterDoc>, cx: StageCx) -> Element {
    match doc.0.needs.clone() {
        Some(needs) => rsx! { Unshown { doc: doc.clone(), needs, cx: cx.clone() } },
        None => rsx! { PictureContent { doc: doc.clone(), cx: cx.clone() } },
    }
}

/// A file with no picture because the plugin that decodes it is not installed: its facts and the
/// row that names the package, as a recording without a player shows.
#[component]
fn Unshown(doc: Held<RasterDoc>, needs: crate::Need, cx: StageCx) -> Element {
    let run = cx.run;
    let facts: Vec<ds::components::fields::fact_list::Fact> = doc
        .0
        .facts
        .rows()
        .iter()
        .map(|row| {
            ds::components::fields::fact_list::Fact::new(row.label.label(), row.value.as_str())
        })
        .collect();
    rsx! {
        div { class: "viewer-peek",
            div { class: "viewer-peek-body",
                EmptyState {
                    icon: Icon::File,
                    title: "This picture cannot be shown yet".to_owned(),
                    description: Some(TextLine::from(format!("{}: {}", needs.fact.label.label(), needs.fact.value.as_str()))),
                    action: rsx! {
                        div { class: "viewer-failed-actions",
                            if let Some(helper) = needs.helper {
                                Button {
                                    label: "Install…",
                                    onclick: move |_| run.call(Command::Install(helper)),
                                }
                            }
                        }
                    },
                }
                div { class: "viewer-peek-facts", FactList { facts } }
            }
        }
    }
}

/// How near an edge of the crop rectangle still takes hold of it, in logical pixels of the room.
const CROP_REACH: f32 = 10.0;

/// The picture as the stage lays it out: the part of the file's picture that is kept, and how far
/// it is turned (by the stage and by the person's edits).
fn laid_out(cx: &StageCx, stage: &RasterStage, size: PixelSize) -> (PixelSize, QuarterTurn) {
    let adjust = cx.picture.adjust();
    (adjust.kept(size).size, turn_of(stage).then(adjust.turn))
}

/// The point of the picture as shown that is under (`at`, in the room) now.
fn under(
    cx: &StageCx,
    stage: &RasterStage,
    size: PixelSize,
    area: Area,
    at: (f32, f32),
) -> DocPoint {
    let (kept, turn) = laid_out(cx, stage, size);
    let shown = scale_of(stage, fit(kept, turn, area));
    point_under(centre_of(stage, kept, turn), shown, area, at)
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
        let shown = scale_of(stage, fit(kept, turn, area));
        // A pinch and a wheel turn under the primary modifier zoom alike: `by` thousandths, about the pointer.
        let zoom_by = |by: i32, at: Point| {
            let zoom = Zoom::scaled(Permille(
                u32::try_from(i64::from(shown.0) * (1000 + i64::from(by)) / 1000)
                    .unwrap_or(Zoom::MIN_SCALE.0),
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

/// Select | Pan, as Preview's tool control, and Crop beside them when the picture can be saved with
/// changes: a drag pans a zoomed picture under Pan (or while Space is held) and moves the corners of
/// the crop rectangle under Crop. It sits on the titlebar's trailing side, where the window can show
/// a mode that stays. The tip is the owner's terse `Name  Key`: the key is H, which switches the
/// tool.
#[component]
pub(super) fn PointerModes(tool: Tool, tools: Vec<Tool>, onpick: EventHandler<Tool>) -> Element {
    let choices: Vec<Choice<Tool>> = tools
        .iter()
        .map(|offered| Choice::new(*offered, offered.label()))
        .collect();
    rsx! {
        Tooltip {
            text: "Pointer Tool",
            shortcut: Some(Shortcut(vec![ShortcutKey::Char('h')])),
            div {
                class: "viewer-modes",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                SegmentedControl::<Tool> {
                    label: "Pointer mode",
                    choices,
                    tracking: Tracking::SelectOne(tool),
                    onchange: move |picked: Tool| {
                        if picked != tool {
                            onpick.call(picked);
                        }
                    },
                }
            }
        }
    }
}
