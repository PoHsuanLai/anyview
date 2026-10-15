//! The trim bar over a recording: a track the length of the recording with the part that is kept
//! framed in yellow between two handles, the playhead, the two times, Play, Cancel and Trim. Where
//! a press lands on the track is arithmetic here; what a handle may do (stay inside, stay apart,
//! show its frame) is the trim machine's, which the view only sends the handle's new place.

use super::panel::clock_text;
use crate::families::view::StageCx;
use crate::{Command, MediaStage, Stage, StageCommand, TrimEdge, TrimIn, TrimWindow};
use anyview_core::{MediaLength, MediaTime};
use dioxus::prelude::*;
use ds::components::controls::button::Button;
use ds::components::controls::button_model::Answers;
use ds::host::captured::{CapturedPointer, PointerPhase};
use ds::host::measure::use_rect;
use ds::host::pointer_capture::{PointerHold, use_pointer_capture};

/// The time under `x`, a place across a track that starts at `left` and is `width` wide, of a
/// recording `whole` long; the start or the end for a place beyond the track.
pub(super) fn time_at(x: f32, left: f32, width: f32, whole: MediaLength) -> MediaTime {
    if width <= 0.0 {
        return MediaTime::default();
    }
    let across = f64::from(((x - left) / width).clamp(0.0, 1.0));
    MediaTime((across * whole.0.0 as f64).round() as u64)
}

/// The handle a press at `at` takes hold of: the one it is nearer to, the start on a tie.
pub(super) fn nearest(at: MediaTime, window: TrimWindow) -> TrimEdge {
    if at.0.abs_diff(window.start.0) <= at.0.abs_diff(window.end.0) {
        TrimEdge::Start
    } else {
        TrimEdge::End
    }
}

/// `at` as a percentage of a track that is `whole` long.
fn percent(at: MediaTime, whole: MediaLength) -> f64 {
    match whole.0.0 {
        0 => 0.0,
        total => at.0.min(total) as f64 / total as f64 * 100.0,
    }
}

/// The bar, while the trim is being chosen.
#[component]
pub(super) fn TrimBar(cx: StageCx) -> Element {
    let track = use_rect();
    let mut held = use_signal(|| None::<TrimEdge>);
    let mut hold = use_signal(|| PointerHold::Local);
    let trim_in = cx.trim_in;
    let handles = cx.trim.handles();
    let whole = handles.map(|(whole, _)| whole);
    let capture = use_pointer_capture(move |pointer: CapturedPointer| {
        let (Some(edge), Some(whole), Some(rect)) = (held(), whole, track.rect()) else {
            return;
        };
        match pointer.phase {
            PointerPhase::Drag => trim_in.call(TrimIn::Move {
                edge,
                to: time_at(pointer.at.x.0, rect.origin.x.0, rect.size.width.0, whole),
            }),
            PointerPhase::Release => held.set(None),
            PointerPhase::Press => {}
        }
    });
    let Some((whole, window)) = handles else {
        return rsx! {};
    };
    let (playing, at) = match &cx.stage {
        Stage::Media(stage) => (
            matches!(stage, MediaStage::Playing { .. }),
            stage.position().unwrap_or_default(),
        ),
        Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => (false, MediaTime::default()),
    };
    let (start, end) = (percent(window.start, whole), percent(window.end, whole));
    let head = percent(at, whole);
    let kept = end - start;
    let times = format!(
        "{} \u{2013} {}",
        clock_text(window.start),
        clock_text(window.end)
    );
    let run = cx.run;
    let placed = move |x: f32| {
        track
            .rect()
            .map(|rect| time_at(x, rect.origin.x.0, rect.size.width.0, whole))
    };
    let pressed = placed;
    let moved = placed;
    rsx! {
        div { class: "viewer-trim",
            div {
                class: "viewer-trim-bar",
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                ondoubleclick: move |event: MouseEvent| event.stop_propagation(),
                Button {
                    label: if playing { "Pause" } else { "Play" },
                    onclick: move |_| run.call(Command::Stage(StageCommand::TogglePlayback)),
                }
                div {
                    class: "viewer-trim-track",
                    role: "group",
                    "aria-label": "Part to keep",
                    "data-dragging": if held().is_some() { "on" } else { "off" },
                    onmounted: move |event| {
                        track.on_mounted(event.clone());
                        capture.on_mounted(event);
                    },
                    onpointerdown: move |event: PointerEvent| {
                        event.stop_propagation();
                        if event.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
                            return;
                        }
                        let point = event.client_coordinates();
                        let Some(to) = pressed(point.x as f32) else { return };
                        let edge = nearest(to, window);
                        held.set(Some(edge));
                        trim_in.call(TrimIn::Move { edge, to });
                        hold.set(capture.begin());
                    },
                    onpointermove: move |event: PointerEvent| {
                        let (Some(edge), PointerHold::Local) = (held(), hold()) else { return };
                        let point = event.client_coordinates();
                        if let Some(to) = moved(point.x as f32) {
                            trim_in.call(TrimIn::Move { edge, to });
                        }
                    },
                    onpointerup: move |_| {
                        if hold() == PointerHold::Local {
                            held.set(None);
                        }
                    },
                    div {
                        class: "viewer-trim-kept",
                        style: "left:{start}%; width:{kept}%",
                    }
                    div {
                        class: "viewer-trim-handle",
                        "data-edge": "start",
                        role: "slider",
                        "aria-label": "Start",
                        style: "left:{start}%",
                    }
                    div {
                        class: "viewer-trim-handle",
                        "data-edge": "end",
                        role: "slider",
                        "aria-label": "End",
                        style: "left:{end}%",
                    }
                    div { class: "viewer-trim-head", style: "left:{head}%" }
                }
                span { class: "viewer-trim-times", "{times}" }
                Button {
                    label: "Cancel",
                    answers: Answers::Escape,
                    onclick: move |_| trim_in.call(TrimIn::Cancel),
                }
                Button {
                    label: "Trim",
                    answers: Answers::Return,
                    onclick: move |_| trim_in.call(TrimIn::Apply),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(seconds: u64) -> MediaTime {
        MediaTime::from_secs(seconds)
    }

    #[test]
    fn a_place_across_the_track_is_a_time_of_the_recording_and_stops_at_its_ends() {
        let whole = MediaLength(secs(200));
        // name, x, left, width, the time
        const CASES: &[(&str, f32, f32, f32, u64)] = &[
            ("the start", 100.0, 100.0, 400.0, 0),
            ("a quarter", 200.0, 100.0, 400.0, 50),
            ("the middle", 300.0, 100.0, 400.0, 100),
            ("the end", 500.0, 100.0, 400.0, 200),
            ("before the track", 20.0, 100.0, 400.0, 0),
            ("past the track", 900.0, 100.0, 400.0, 200),
            ("a track with no width", 150.0, 100.0, 0.0, 0),
        ];
        for (name, x, left, width, want) in CASES {
            assert_eq!(time_at(*x, *left, *width, whole), secs(*want), "{name}");
        }
    }

    #[test]
    fn a_press_takes_the_handle_it_is_nearer_to() {
        let window = TrimWindow {
            start: secs(20),
            end: secs(60),
        };
        const CASES: &[(&str, u64, TrimEdge)] = &[
            ("before the start", 5, TrimEdge::Start),
            ("on the start", 20, TrimEdge::Start),
            ("just inside the start", 39, TrimEdge::Start),
            ("halfway ties to the start", 40, TrimEdge::Start),
            ("just past halfway", 41, TrimEdge::End),
            ("past the end", 90, TrimEdge::End),
        ];
        for (name, at, want) in CASES {
            assert_eq!(nearest(secs(*at), window), *want, "{name}");
        }
    }
}
