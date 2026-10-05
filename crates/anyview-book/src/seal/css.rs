//! Stylesheets made safe to put in a sealed page: no import, no web font, no address that is not
//! an inlined `data:` URL.

use super::assets::Assets;
use crate::zip_path::resolve;

/// What a `url()` that cannot be inlined becomes: an empty resource.
const EMPTY_URL: &str = "url(\"data:,\")";

/// The markers the scanner stops at, lower-case.
const MARKERS: &[&str] = &["/*", "@import", "@font-face", "url(", "<"];

/// `css`, a stylesheet or a `style` attribute of a document in `directory`, with its comments,
/// imports and font faces removed and each `url()` replaced by the file's `data:` URL when the
/// package holds it and by an empty one otherwise.
pub(super) fn seal_css(css: &str, directory: &str, assets: &dyn Assets) -> String {
    let lower = css.to_ascii_lowercase();
    let mut out = String::with_capacity(css.len());
    let mut at = 0;
    while at < css.len() {
        let next = MARKERS
            .iter()
            .filter_map(|marker| lower[at..].find(marker).map(|found| (at + found, *marker)))
            .min_by_key(|(position, _)| *position);
        let Some((position, marker)) = next else {
            out.push_str(&css[at..]);
            break;
        };
        out.push_str(&css[at..position]);
        at = match marker {
            "/*" => skip_to(css, position + 2, "*/"),
            "@import" => skip_to(css, position + 7, ";"),
            "@font-face" => skip_block(css, position),
            "url(" => url(css, position, directory, assets, &mut out),
            _ => {
                out.push_str("\\3c ");
                position + 1
            }
        };
    }
    out
}

/// The position just past the next `end` at or after `from`, or the end of `css`.
fn skip_to(css: &str, from: usize, end: &str) -> usize {
    css[from..]
        .find(end)
        .map_or(css.len(), |found| from + found + end.len())
}

/// The position just past the `{ ... }` block that starts after `from`.
fn skip_block(css: &str, from: usize) -> usize {
    let Some(open) = css[from..].find('{') else {
        return css.len();
    };
    let mut depth = 0_u32;
    for (offset, c) in css[from + open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return from + open + offset + 1;
                }
            }
            _ => {}
        }
    }
    css.len()
}

/// Writes the replacement of the `url(` at `position` and returns the position after its `)`.
fn url(
    css: &str,
    position: usize,
    directory: &str,
    assets: &dyn Assets,
    out: &mut String,
) -> usize {
    let from = position + 4;
    let end = css[from..]
        .find(')')
        .map_or(css.len(), |found| from + found);
    let argument = css[from..end]
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();
    let inlined = resolve(directory, argument).and_then(|entry| assets.data_url(&entry));
    match inlined {
        Some(data) => {
            out.push_str("url(\"");
            out.push_str(&data);
            out.push_str("\")");
        }
        None => out.push_str(EMPTY_URL),
    }
    (end + 1).min(css.len())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::collections::HashMap;

    pub(in crate::seal) struct Files(pub HashMap<&'static str, &'static str>);

    impl Assets for Files {
        fn data_url(&self, entry: &str) -> Option<String> {
            self.0.get(entry).map(|data| (*data).to_owned())
        }
        fn text(&self, entry: &str) -> Option<String> {
            self.0.get(entry).map(|text| (*text).to_owned())
        }
    }

    #[test]
    fn a_stylesheet_keeps_its_rules_and_loses_what_would_reach_out() {
        let files = Files(HashMap::from([("css/bg.png", "data:image/png;base64,AA")]));
        // name, css, sealed
        const CASES: &[(&str, &str, &str)] = &[
            ("plain rules", "p { color: red }", "p { color: red }"),
            ("comments go", "a /* x */ b", "a  b"),
            ("imports go", "@import url(http://x/y.css); p{}", " p{}"),
            (
                "font faces go",
                "@font-face { font-family: a; src: url(a.woff) } p{}",
                " p{}",
            ),
            (
                "a held image is inlined",
                "p { background: URL('bg.png') }",
                "p { background: url(\"data:image/png;base64,AA\") }",
            ),
            (
                "a remote image is emptied",
                "p { background: url(https://x/y.png) }",
                "p { background: url(\"data:,\") }",
            ),
            (
                "markup cannot close the style",
                "a::after{content:'</style>'}",
                "a::after{content:'\\3c /style>'}",
            ),
        ];
        for (name, css, want) in CASES {
            assert_eq!(seal_css(css, "css/", &files), *want, "{name}");
        }
    }
}
