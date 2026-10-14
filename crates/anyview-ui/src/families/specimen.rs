//! A font's specimen on the stage, laid out as Font Book does: the family name large, the pangram,
//! the alphabet and digits, and a short line at a ramp of sizes. Every line is the face's own
//! outlines as an inline SVG filled with the window's ink (`currentColor`), so it follows light and
//! dark, and no font has to be loaded to draw it. The `data-ds-svg` marker is quire's word for a
//! component that draws its own vector, which its markup lint then allows.

use anyview_font::{EM, Specimen, SpecimenLine};
use dioxus::prelude::*;

/// The size, in px, the family name is set at.
const TITLE_PX: u32 = 48;

/// The size the pangram is set at.
const PANGRAM_PX: u32 = 24;

/// The size the alphabet and digit lines are set at.
const ALPHABET_PX: u32 = 36;

/// The sizes the ramp line is set at, smallest first.
const RAMP_PX: [u32; 6] = [12, 18, 24, 36, 48, 72];

/// Whether a specimen has anything to draw.
pub(crate) fn is_drawn(specimen: &Specimen) -> bool {
    !specimen.lines.is_empty()
        || specimen.title.is_some()
        || specimen.pangram.is_some()
        || specimen.ramp.is_some()
}

/// The sheet of `specimen`.
#[component]
pub(crate) fn SpecimenSheet(specimen: Specimen) -> Element {
    rsx! {
        div { class: "viewer-specimen", role: "group", aria_label: "Specimen",
            if let Some(title) = specimen.title.clone() {
                Line { line: title, px: TITLE_PX }
            }
            if let Some(pangram) = specimen.pangram.clone() {
                Line { line: pangram, px: PANGRAM_PX }
            }
            for (at , line) in specimen.lines.iter().enumerate() {
                Line { key: "{at}", line: line.clone(), px: ALPHABET_PX }
            }
            if let Some(ramp) = specimen.ramp.clone() {
                div { class: "viewer-specimen-ramp",
                    for px in RAMP_PX {
                        div { key: "{px}", class: "viewer-specimen-step",
                            span { class: "viewer-specimen-size", "{px}" }
                            Line { line: ramp.clone(), px }
                        }
                    }
                }
            }
        }
    }
}

/// One line of outlines set at `px` to the em.
#[component]
fn Line(line: SpecimenLine, px: u32) -> Element {
    // Rounded up so a line is never drawn a pixel smaller than its outlines.
    let wide = (px * line.width.max(1)).div_ceil(EM);
    let high = (px * line.height.max(1)).div_ceil(EM);
    let (width, height) = (line.width.max(1), line.height.max(1));
    rsx! {
        svg {
            class: "viewer-specimen-line",
            "data-ds-svg": "specimen",
            role: "img",
            "aria-label": "{line.text}",
            view_box: "0 0 {width} {height}",
            width: "{wide}",
            height: "{high}",
            path { d: "{line.path}", fill: "currentColor" }
        }
    }
}
