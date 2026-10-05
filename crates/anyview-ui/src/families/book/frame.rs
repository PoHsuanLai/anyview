//! A section as the document of a sealed frame. The frame inherits nothing from the window, so its
//! document carries the design system's stylesheet and the root's attributes, which makes every
//! colour, size and face a token there as it is outside. The section's markup was sealed by
//! `anyview-book`: no script, no frame, nothing that loads from the network.

use super::doc::{Layout, SectionPage};
use crate::families::view::FrameLook;

/// The page's own rules: how a chapter reads and how a comic page fits the room.
const PAGE: &str = include_str!("page.css");

/// The reading page of the text family, which a chapter reads in.
const READING: &str = include_str!("../text/page.css");

/// The frame's document for `page`, dressed in the window's look. The book's own styles come
/// last, so a chapter looks as its author set it where the book says so.
pub(super) fn document(page: &SectionPage, look: &FrameLook) -> String {
    let article = match page.layout {
        Layout::Chapter => "viewer-page viewer-book",
        Layout::Picture => "viewer-comic",
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}</style><style>{READING}</style><style>{PAGE}</style><style>{}</style></head><body class=\"ds\" style=\"margin:0\" {}><div class=\"viewer-ground\"><article class=\"{article}\">{}</article></div></body></html>",
        ds::stylesheet(),
        page.styles,
        look.attributes,
        page.body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::SectionIndex;

    #[test]
    fn the_frame_holds_the_sections_markup_in_the_windows_look() {
        let page = SectionPage {
            section: SectionIndex(0),
            layout: Layout::Chapter,
            styles: "h1 { color: blue }".to_owned(),
            body: "<h1>One</h1>".to_owned(),
        };
        let look = FrameLook {
            attributes: "data-theme=\"dark\"".to_owned(),
        };
        let html = document(&page, &look);
        assert!(html.contains("<body class=\"ds\" style=\"margin:0\" data-theme=\"dark\">"));
        assert!(html.contains("<article class=\"viewer-page viewer-book\"><h1>One</h1></article>"));
        assert!(html.contains("h1 { color: blue }"));
        assert!(!html.contains("<script"));
    }
}
