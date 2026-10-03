//! The page of a Markdown file as the document of a sealed frame. A frame inherits nothing from
//! the window, so its document carries the design system's stylesheet and the window root's own
//! attributes, which makes every colour, size and face a token there as it is outside. The
//! rendered HTML has no script, no raw HTML of the source and no link a frame may follow.

use super::super::view::FrameLook;
use anyview_text::Rendered;

/// The page's own rules: what the rendered elements look like, in tokens.
const PAGE: &str = include_str!("page.css");

/// The frame's document for `rendered`, dressed in the window's look.
pub(super) fn document(rendered: &Rendered, look: &FrameLook) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}</style><style>{PAGE}</style><style>{}</style></head><body class=\"ds\" style=\"margin:0\" {}><div class=\"viewer-ground\"><article class=\"viewer-page\">{}</article></div></body></html>",
        ds::stylesheet(),
        crate::families::TOKEN_CSS,
        look.attributes,
        rendered.html,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_text::{NoFiles, RenderEnv, render};

    #[test]
    fn the_page_wears_the_windows_attributes_and_holds_the_rendered_body() {
        let env = RenderEnv {
            base: None,
            files: &NoFiles,
            highlighter: None,
        };
        let rendered = render("# Title\n\nBody *text*.", &env);
        let look = FrameLook {
            attributes: "data-theme=\"dark\" data-accent=\"blue\"".to_owned(),
        };
        let page = document(&rendered, &look);
        assert!(page.contains(
            "<body class=\"ds\" style=\"margin:0\" data-theme=\"dark\" data-accent=\"blue\">"
        ));
        assert!(page.contains("<article class=\"viewer-page\"><h1"));
        assert!(page.contains("<em>text</em>"));
        assert!(!page.contains("<script"));
    }
}
