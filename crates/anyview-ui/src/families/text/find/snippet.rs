//! The words around a hit, clipped from its line for the palette's list.

/// The words around a hit, for the palette's list: a clip of its line and where in the clip the
/// match is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    /// The clip.
    pub text: String,
    /// The match inside it, as byte offsets.
    pub matched: std::ops::Range<usize>,
    /// The character the match starts at in the line it was clipped from, counting from one; zero
    /// when the clip knows no line.
    pub column: usize,
}

/// How much of a line is kept before a match and after it, in bytes.
const BEFORE: usize = 32;
const AFTER: usize = 72;

/// `line` clipped to the words around the bytes `from..to`: leading space gone, an ellipsis where
/// the line was cut, the match located in the result. Offsets that are not on character edges
/// (the highlighter and the search disagreeing) give the start of the line with no match marked.
pub(crate) fn clip(line: &str, from: usize, to: usize) -> Snippet {
    let from_ok = from <= to && to <= line.len();
    if !from_ok || !line.is_char_boundary(from) || !line.is_char_boundary(to) {
        let text: String = line.trim().chars().take(BEFORE + AFTER).collect();
        return Snippet {
            text,
            matched: 0..0,
            column: 0,
        };
    }
    let mut start = from.saturating_sub(BEFORE);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let lead = line[start..from].len() - line[start..from].trim_start().len();
    start += lead;
    let mut end = to.saturating_add(AFTER).min(line.len());
    while !line.is_char_boundary(end) {
        end += 1;
    }
    let body = line[start..end].trim_end();
    let end = (start + body.len()).max(to);
    let body = &line[start..end];
    let cut_before = start > line.len() - line.trim_start().len();
    let cut_after = end < line.trim_end().len();
    let (head, tail) = (
        if cut_before { "\u{2026}" } else { "" },
        if cut_after { "\u{2026}" } else { "" },
    );
    Snippet {
        text: format!("{head}{body}{tail}"),
        matched: head.len() + (from - start)..head.len() + (to - start),
        column: line[..from].chars().count() + 1,
    }
}
