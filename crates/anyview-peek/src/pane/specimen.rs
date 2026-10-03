//! A font's specimen: each sample line is an inline SVG of the face's own outlines, filled with
//! the pane's ink, so the face shown is exactly the one the file holds. The `data-ds-svg` marker is
//! quire's word for a component that draws its own vector, which its markup lint then allows.

use anyview_font::{FontPeeked, SpecimenLine};
use dioxus::prelude::*;

/// How tall each line stands, in px; its width follows the line's own proportions, and the
/// stylesheet keeps it inside the media box.
const LINE_PX: u32 = 34;

/// The lines of `font`'s specimen, or nothing when it has none (the caller draws a plate).
pub(super) fn lines(font: &FontPeeked) -> Option<Element> {
    let face = font.face.as_ref()?;
    if face.specimen.lines.is_empty() {
        return None;
    }
    let lines = face.specimen.lines.iter().enumerate().map(|(at, line)| {
        rsx! {
            Line { key: "{at}", line: line.clone() }
        }
    });
    Some(rsx! {
        div { class: "anyview-specimen", {lines} }
    })
}

#[component]
fn Line(line: SpecimenLine) -> Element {
    let (width, height) = (line.width.max(1), line.height.max(1));
    // Rounded up so a line is never drawn a pixel narrower than its outlines.
    let wide = (width * LINE_PX).div_ceil(height);
    rsx! {
        svg {
            class: "anyview-specimen-line",
            "data-ds-svg": "specimen",
            role: "img",
            "aria-label": "{line.text}",
            view_box: "0 0 {width} {height}",
            width: "{wide}",
            height: "{LINE_PX}",
            path { d: "{line.path}", fill: "currentColor" }
        }
    }
}
