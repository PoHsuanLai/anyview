//! The crop rectangle drawn over the picture: the picture dimmed outside it, its frame with eight
//! handles and the guides that cut it in thirds while a grip is held, and the bar below with the
//! shapes it can be held to, Cancel and Crop. Where everything is, is arithmetic over the stage's
//! geometry; what a press on it means is the picture edits' (the stage view sends the press).

use super::geometry::room_point;
use crate::families::view::Area;
use crate::{CropAspect, CropBox, CropGrip, CropLean, CropShape, PictureEditIn};
use anyview_core::{DocPoint, Permille};
use dioxus::prelude::*;
use ds::components::controls::button::Button;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, SegmentedControl};

/// The rectangle's corners in the room, in logical pixels: left, top, right, bottom.
fn corners(draft: CropBox, centre: DocPoint, scale: Permille, area: Area) -> (f64, f64, f64, f64) {
    let (left, top) = room_point(
        centre,
        scale,
        area,
        (f64::from(draft.left), f64::from(draft.top)),
    );
    let (right, bottom) = room_point(
        centre,
        scale,
        area,
        (f64::from(draft.right()), f64::from(draft.bottom())),
    );
    (left, top, right, bottom)
}

/// Where a handle sits: the middle of its edge, or its corner.
fn handle_at(grip: CropGrip, (left, top, right, bottom): (f64, f64, f64, f64)) -> (f64, f64) {
    let (middle_x, middle_y) = ((left + right) / 2.0, (top + bottom) / 2.0);
    match grip {
        CropGrip::NorthWest => (left, top),
        CropGrip::North => (middle_x, top),
        CropGrip::NorthEast => (right, top),
        CropGrip::East => (right, middle_y),
        CropGrip::SouthEast => (right, bottom),
        CropGrip::South => (middle_x, bottom),
        CropGrip::SouthWest => (left, bottom),
        CropGrip::West => (left, middle_y),
        CropGrip::Body => (middle_x, middle_y),
    }
}

/// A box of the room as inline geometry.
fn boxed(left: f64, top: f64, width: f64, height: f64) -> String {
    format!(
        "left:{left}px; top:{top}px; width:{}px; height:{}px",
        width.max(0.0),
        height.max(0.0)
    )
}

/// The overlay of the crop tool: `draft` is the rectangle in the picture as shown, drawn where the
/// stage places the picture (`centre` and `scale` in `area`).
#[component]
pub(super) fn CropOverlay(
    draft: CropBox,
    aspect: CropAspect,
    dragging: bool,
    centre: DocPoint,
    scale: Permille,
    area: Area,
    onedit: EventHandler<PictureEditIn>,
) -> Element {
    let at = corners(draft, centre, scale, area);
    let (left, top, right, bottom) = at;
    let (width, height) = (f64::from(area.size.width.0), f64::from(area.size.height.0));
    let dims = [
        boxed(0.0, 0.0, width, top),
        boxed(0.0, bottom, width, height - bottom),
        boxed(0.0, top, left, bottom - top),
        boxed(right, top, width - right, bottom - top),
    ];
    let (columns, rows) = draft.thirds();
    let guide_columns = columns.map(|column| {
        let (x, _) = room_point(centre, scale, area, (f64::from(column), 0.0));
        boxed(x, top, 0.0, bottom - top)
    });
    let guide_rows = rows.map(|row| {
        let (_, y) = room_point(centre, scale, area, (0.0, f64::from(row)));
        boxed(left, y, right - left, 0.0)
    });
    let shapes: Vec<Choice<CropShape>> = CropShape::ALL
        .iter()
        .map(|shape| Choice::new(*shape, shape.label()))
        .collect();
    let leans = vec![
        Choice::new(CropLean::Wide, CropLean::Wide.label()),
        Choice::new(CropLean::Tall, CropLean::Tall.label()),
    ];
    rsx! {
        div { class: "viewer-crop", "data-dragging": if dragging { "on" } else { "off" },
            for style in dims {
                div { class: "viewer-crop-dim", style }
            }
            div { class: "viewer-crop-frame", style: boxed(left, top, right - left, bottom - top) }
            for style in guide_columns {
                div { class: "viewer-crop-guide", "data-axis": "column", style }
            }
            for style in guide_rows {
                div { class: "viewer-crop-guide", "data-axis": "row", style }
            }
            for grip in CropGrip::HANDLES {
                {
                    let (x, y) = handle_at(grip, at);
                    rsx! {
                        div {
                            class: "viewer-crop-handle",
                            "data-grip": grip.slug(),
                            style: "left:{x}px; top:{y}px",
                        }
                    }
                }
            }
        }
        div { class: "viewer-crop-dock",
            div {
                class: "viewer-crop-bar",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                SegmentedControl::<CropShape> {
                    label: "Crop shape",
                    choices: shapes,
                    tracking: Tracking::SelectOne(aspect.shape),
                    onchange: move |shape: CropShape| {
                        onedit.call(PictureEditIn::Hold(CropAspect { shape, lean: aspect.lean }));
                    },
                }
                SegmentedControl::<CropLean> {
                    label: "Crop orientation",
                    choices: leans,
                    tracking: Tracking::SelectOne(aspect.lean),
                    onchange: move |lean: CropLean| {
                        onedit.call(PictureEditIn::Hold(CropAspect { shape: aspect.shape, lean }));
                    },
                }
                Button { label: "Cancel", onclick: move |_| onedit.call(PictureEditIn::CropOff) }
                Button { label: "Crop", onclick: move |_| onedit.call(PictureEditIn::Apply) }
            }
        }
    }
}
