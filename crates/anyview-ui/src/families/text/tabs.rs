//! Tabs in a line of text. The fixed-pitch face has no glyph for a tab (it would draw as a missing-
//! glyph box) and the layout engine has no `tab-size`, so a tab is drawn as the spaces up to the
//! next tab stop, as an editor draws it. The text itself is untouched: a hit, a selection and a
//! copy still refer to the real tab, and only what is drawn is expanded. The stops are every
//! `TAB_WIDTH` columns for code and plain text alike, counted in characters of the fixed-pitch
//! face.

/// The columns between tab stops.
pub(super) const TAB_WIDTH: usize = 4;

/// The spaces a tab at display column `col` is drawn as.
fn span(col: usize) -> usize {
    TAB_WIDTH - col % TAB_WIDTH
}

/// `text` as drawn, its tabs expanded to the next stop. `col` is the display column the text
/// starts at, and is left at the column it ends at, so a line drawn in pieces lines up.
pub(super) fn expand(text: &str, col: &mut usize) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            let spaces = span(*col);
            out.extend(std::iter::repeat_n(' ', spaces));
            *col += spaces;
        } else {
            out.push(ch);
            *col += 1;
        }
    }
    out
}

/// How many display columns `text` takes, tabs expanded.
pub(super) fn width(text: &str) -> usize {
    let mut col = 0;
    for ch in text.chars() {
        col += if ch == '\t' { span(col) } else { 1 };
    }
    col
}

/// The byte offset in the drawn line of the byte offset `real` in `line`.
pub(super) fn drawn_offset(line: &str, real: usize) -> usize {
    let (mut col, mut drawn) = (0, 0);
    for (at, ch) in line.char_indices() {
        if at >= real {
            return drawn;
        }
        if ch == '\t' {
            let spaces = span(col);
            drawn += spaces;
            col += spaces;
        } else {
            drawn += ch.len_utf8();
            col += 1;
        }
    }
    drawn + real.saturating_sub(line.len())
}

/// The byte offset in `line` of the byte offset `drawn` in the drawn line; a point inside a
/// tab's spaces is the nearer edge of the tab.
pub(super) fn real_offset(line: &str, drawn: usize) -> usize {
    let (mut col, mut seen) = (0, 0);
    for (at, ch) in line.char_indices() {
        let (size, spaces) = if ch == '\t' {
            let spaces = span(col);
            (spaces, spaces)
        } else {
            (ch.len_utf8(), 1)
        };
        if drawn < seen + size {
            let past_middle = ch == '\t' && (drawn - seen) * 2 > size;
            return if past_middle { at + 1 } else { at };
        }
        seen += size;
        col += spaces;
    }
    line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_is_drawn_as_the_spaces_to_the_next_stop() {
        // name, text, drawn
        const CASES: &[(&str, &str, &str)] = &[
            ("a lone tab", "\t", "    "),
            ("after one glyph", "a\tb", "a   b"),
            ("on a stop", "abcd\te", "abcd    e"),
            ("two in a row", "\t\tx", "        x"),
            ("no tab", "plain", "plain"),
            ("a wide letter counts as one column", "é\tx", "é   x"),
        ];
        for (name, text, drawn) in CASES {
            let mut col = 0;
            assert_eq!(expand(text, &mut col), *drawn, "{name}");
            assert_eq!(col, width(text), "{name}: the column it ends at");
        }
    }

    #[test]
    fn pieces_of_a_line_line_up_as_the_whole_does() {
        let mut col = 0;
        let drawn = format!("{}{}", expand("ab", &mut col), expand("\tc", &mut col));
        assert_eq!(drawn, "ab  c");
    }

    #[test]
    fn offsets_map_between_the_text_and_what_is_drawn() {
        // name, line, real offset, drawn offset
        const CASES: &[(&str, &str, usize, usize)] = &[
            ("before a tab", "a\tb", 1, 1),
            ("after a tab", "a\tb", 2, 4),
            ("past the end", "a\tb", 3, 5),
            ("after two tabs", "\t\tx", 2, 8),
            ("after a wide letter", "é\tx", 3, 5),
        ];
        for (name, line, real, drawn) in CASES {
            assert_eq!(drawn_offset(line, *real), *drawn, "{name}");
            assert_eq!(real_offset(line, *drawn), *real, "{name}: and back");
        }
        // a point inside a tab's spaces goes to the nearer edge
        assert_eq!(real_offset("a\tb", 2), 1);
        assert_eq!(real_offset("a\tb", 3), 2);
    }
}
