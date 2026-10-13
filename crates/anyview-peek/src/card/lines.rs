//! The first lines of text and of code.

use anyview_text::{TOKEN_CLASS_PREFIX, TokenLine};
use dioxus::prelude::*;
use ds::prelude::Word;

/// Plain lines, one block each; an empty line keeps its height with a no-break space.
pub(super) fn plain(lines: &[String]) -> Element {
    rsx! {
        pre { class: "anyview-lines", "data-face": "mono",
            for (at , line) in lines.iter().enumerate() {
                div { key: "{at}", class: "anyview-line", {shown(line)} }
            }
        }
    }
}

/// Highlighted lines: each span in a `tok-<class>` element the stylesheet colours.
pub(super) fn code(lines: &[TokenLine]) -> Element {
    rsx! {
        pre { class: "anyview-lines", "data-face": "mono",
            for line in lines {
                div { key: "{line.number.0}", class: "anyview-line",
                    if line.spans.is_empty() {
                        "\u{a0}"
                    }
                    for (at , span) in line.spans.iter().enumerate() {
                        span { key: "{at}", class: "{TOKEN_CLASS_PREFIX}{span.class.slug()}", "{span.text}" }
                    }
                }
            }
        }
    }
}

fn shown(line: &str) -> &str {
    if line.is_empty() { "\u{a0}" } else { line }
}
