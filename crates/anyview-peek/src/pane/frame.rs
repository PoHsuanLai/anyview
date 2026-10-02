//! A rendered Markdown start, in the sealed frame Blitz gives a foreign document.
//!
//! The HTML is the peek's (raw HTML of the source already shown as text, local images as their alt
//! text, only web, mail and relative links kept). It sits in an `<iframe srcdoc>` whose network
//! is sealed to `data:` and whose links never navigate (quire FINDINGS "Frames and links"), on a
//! paper sheet like a mail's original message: the frame is its own document, so it takes no
//! design tokens and is written in the one scheme every foreign page is shown in.

use dioxus::prelude::*;
use ds::components::lists::preview::content::PANE_MEDIA;

/// The tag `ds-blitz` knows the frame by.
const FRAME_TAG: &str = "anyview-peek-markdown";

/// The frame's own stylesheet: literal values, because a frame's document cannot read the
/// pane's tokens. Dark on white, whatever the scheme.
const FRAME_STYLE: &str = "body{margin:0;padding:12px;background:#fff;color:#1d1d1f;\
font:13px/1.45 sans-serif}h1,h2,h3,h4,h5,h6{margin:0 0 6px;line-height:1.25}\
h1{font-size:20px}h2{font-size:17px}h3{font-size:15px}p,ul,ol,pre,blockquote,table{margin:0 0 8px}\
pre,code{font-family:monospace;font-size:12px}pre{padding:8px;background:#f2f2f4}\
blockquote{padding-left:10px;border-left:3px solid #d0d0d6;color:#555}\
a{color:#0a58ca}th,td{padding:2px 8px;border:1px solid #d0d0d6}img{max-width:100%}";

/// `html` (a body fragment) as a whole document.
fn document(html: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>{FRAME_STYLE}</style></head>\
<body>{html}</body></html>"
    )
}

/// The start of a Markdown document, rendered.
#[component]
pub(super) fn Frame(html: String) -> Element {
    let srcdoc = document(&html);
    rsx! {
        iframe {
            class: "anyview-frame",
            "data-frame-tag": FRAME_TAG,
            title: "Markdown preview",
            srcdoc,
            style: "width:{PANE_MEDIA.width.0}px;height:{PANE_MEDIA.height.0}px",
        }
    }
}
