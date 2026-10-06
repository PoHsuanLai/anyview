//! How deeply an XML document nests, counted without building it.
//!
//! XML parsers recurse once per open element, so a document of a hundred thousand nested tags
//! overflows the stack of whoever parses it. Counting the nesting first, in one flat pass, lets a
//! reader refuse such a document before it reaches a parser.

/// The deepest nesting of elements a document may have to be parsed: beyond what any real SVG
/// or book package writes, far below what a thread's stack holds.
pub const MAX_XML_DEPTH: usize = 128;

/// Whether `text` opens more than `limit` elements at once, counting from tags alone: comments,
/// CDATA, processing instructions and declarations are skipped, quoted attribute values are not
/// searched for tags, and an element that closes itself (`<a/>`) does not nest. A closing tag with
/// nothing open is ignored; the parser reports the mismatch.
pub fn nests_deeper_than(text: &str, limit: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0_usize;
    let mut at = 0;
    while let Some(found) = bytes
        .get(at..)
        .and_then(|rest| rest.iter().position(|b| *b == b'<'))
    {
        at += found;
        let rest = &bytes[at..];
        if rest.starts_with(b"<!--") {
            at += skip_past(rest, 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            at += skip_past(rest, 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            at += skip_past(rest, 2, b"?>");
        } else if rest.starts_with(b"<!") {
            at += tag_end(rest);
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            at += tag_end(rest);
        } else {
            let end = tag_end(rest);
            if rest.get(end.saturating_sub(2)) != Some(&b'/') {
                depth += 1;
                if depth > limit {
                    return true;
                }
            }
            at += end;
        }
    }
    false
}

/// The offset in `bytes` just past the first `end` at or after `from`, or the length.
fn skip_past(bytes: &[u8], from: usize, end: &[u8]) -> usize {
    bytes
        .get(from..)
        .and_then(|rest| rest.windows(end.len()).position(|window| window == end))
        .map_or(bytes.len(), |found| from + found + end.len())
}

/// The offset in `bytes` just past the `>` that closes the tag at its start, not counting a `>`
/// inside a quoted value, or the length.
fn tag_end(bytes: &[u8]) -> usize {
    let mut quote: Option<u8> = None;
    for (offset, byte) in bytes.iter().enumerate().skip(1) {
        match (quote, *byte) {
            (Some(open), b) if b == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'>') => return offset + 1,
            (None, _) => {}
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_is_counted_from_tags_alone() {
        let deep = |n: usize| format!("{}{}", "<g>".repeat(n), "</g>".repeat(n));
        // name, text, limit, too deep
        let cases: Vec<(&str, String, usize, bool)> = vec![
            ("at the limit", deep(3), 3, false),
            ("one past the limit", deep(4), 3, true),
            ("never closed", "<a>".repeat(10), 3, true),
            ("siblings do not nest", "<a></a>".repeat(100), 1, false),
            ("self-closing does not nest", "<a/>".repeat(100), 0, false),
            (
                "self-closing with attributes",
                "<a b='1' />".repeat(10),
                0,
                false,
            ),
            (
                "a comment hides tags",
                format!("<!--{}-->", "<g>".repeat(50)),
                3,
                false,
            ),
            (
                "cdata hides tags",
                format!("<a><![CDATA[{}]]></a>", "<g>".repeat(50)),
                3,
                false,
            ),
            (
                "a processing instruction hides tags",
                "<?x <g><g><g><g> ?><a/>".to_owned(),
                1,
                false,
            ),
            (
                "a quoted > does not end the tag",
                "<a b='>' c=\"<d><d><d>\"/>".to_owned(),
                0,
                false,
            ),
            (
                "a doctype with entities",
                "<!DOCTYPE a [<!ENTITY e \"<b><b>\">]><a/>".to_owned(),
                0,
                false,
            ),
            (
                "closers without openers",
                "</a></a></a><a>".to_owned(),
                1,
                false,
            ),
            ("not xml at all", "no tags here".to_owned(), 0, false),
        ];
        for (name, text, limit, want) in cases {
            assert_eq!(nests_deeper_than(&text, limit), want, "{name}");
        }
    }

    #[test]
    fn a_hundred_thousand_open_tags_are_counted_at_once() {
        let text = "<a>".repeat(100_000);
        assert!(nests_deeper_than(&text, MAX_XML_DEPTH));
    }
}
