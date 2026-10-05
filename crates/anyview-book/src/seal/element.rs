//! What a sealed chapter may contain: the elements and attributes that are written, and how an
//! address is judged.

/// The elements that are written. Anything else is left out, its text kept.
pub(super) const ELEMENTS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "article",
    "aside",
    "b",
    "bdi",
    "bdo",
    "blockquote",
    "br",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "i",
    "img",
    "ins",
    "kbd",
    "li",
    "main",
    "mark",
    "nav",
    "ol",
    "p",
    "pre",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "section",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "u",
    "ul",
    "var",
    "wbr",
];

/// The elements with no closing tag.
pub(super) const VOID: &[&str] = &["br", "col", "hr", "img", "wbr"];

/// The elements whose contents are left out with them.
pub(super) const DROPPED_WITH_CONTENT: &[&str] = &[
    "applet", "audio", "canvas", "embed", "iframe", "noscript", "object", "template", "video",
];

/// The elements of other languages whose contents are left out, except an SVG `image`.
pub(super) const FOREIGN: &[&str] = &["math", "svg"];

/// The attributes that are written, on any element. `style` is sealed, `href` and `src` are judged.
pub(super) const ATTRIBUTES: &[&str] = &[
    "alt", "class", "colspan", "dir", "headers", "id", "lang", "rowspan", "scope", "span", "start",
    "style", "title", "value",
];

/// Whether `size` is a plain length: digits, a point, a percent sign.
pub(super) fn is_length(size: &str) -> bool {
    !size.is_empty()
        && size
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '%'))
}

/// Whether a link to `target` may be written: a fragment, or a web or mail address. The frame
/// follows none of them; the rule keeps `javascript:` and its kin out of the markup.
pub(super) fn is_safe_link(target: &str) -> bool {
    let target = target.trim();
    if target.starts_with('#') {
        return true;
    }
    let lower = target.to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_judged_by_their_scheme() {
        // name, target, allowed
        const CASES: &[(&str, &str, bool)] = &[
            ("fragment", "#note1", true),
            ("web", "https://example.com", true),
            ("web in capitals", "HTTP://example.com", true),
            ("mail", "mailto:a@b.c", true),
            ("script", "javascript:alert(1)", false),
            ("padded script", "  javascript:alert(1)", false),
            ("data", "data:text/html,x", false),
            ("another chapter", "ch2.xhtml", false),
        ];
        for (name, target, want) in CASES {
            assert_eq!(is_safe_link(target), *want, "{name}");
        }
    }
}
