//! How many lines of a text fit a page when long lines wrap. The stylesheet does the wrapping (a
//! line breaks where the view's width ends); this counts the rows a line is likely to take, so
//! that a page step moves by what is on screen and the window reads no fewer lines than fill it.
//! The count is an estimate from the width of one glyph of the fixed-pitch face: a line that
//! breaks one row earlier or later than estimated makes a page step a line short or long, and
//! nothing else.

use crate::Wrap;
use anyview_text::TokenLine;

/// The width of one glyph of the code face at the viewer's text size, in logical pixels.
pub(super) const GLYPH_PX: f32 = 7.4;

/// The width a row gives up to the side padding (`--s-20` at each end), the line number column
/// (`--s-36`) and the code's own padding (`--s-8`), in logical pixels.
pub(super) const GUTTER_PX: f32 = 84.0;

/// How many glyphs a row holds in a room `width` logical pixels wide; at least one.
pub(super) fn columns(width: f32) -> u32 {
    // A positive, finite width over a positive constant: truncating to whole glyphs is the point.
    ((width - GUTTER_PX) / GLYPH_PX).floor().max(1.0) as u32
}

/// How many rows a line of `glyphs` characters takes in rows of `columns`: one when it is empty.
pub(super) fn rows_of(glyphs: usize, columns: u32) -> u32 {
    let columns = columns.max(1) as usize;
    u32::try_from(glyphs.max(1).div_ceil(columns)).unwrap_or(u32::MAX)
}

/// How many of `lines`, from the first, fit `rows` rows of a room `width` wide: at least one.
/// Without wrapping every line is one row, so it is `rows`.
pub(super) fn lines_per_page(lines: &[TokenLine], rows: u32, width: f32, wrap: Wrap) -> u32 {
    match wrap {
        Wrap::Off => rows.max(1),
        Wrap::On => {
            let columns = columns(width);
            let mut used = 0_u32;
            let mut fitted = 0_u32;
            for line in lines {
                used = used.saturating_add(rows_of(line.text().chars().count(), columns));
                if used > rows {
                    break;
                }
                fitted += 1;
            }
            fitted.max(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::LineIndex;
    use anyview_text::{TokenClass, TokenSpan};

    fn line(glyphs: usize) -> TokenLine {
        TokenLine {
            number: LineIndex(0),
            spans: vec![TokenSpan {
                class: TokenClass::Plain,
                text: "x".repeat(glyphs),
            }],
        }
    }

    #[test]
    fn a_line_takes_as_many_rows_as_its_glyphs_fill() {
        // name, glyphs, glyphs a row holds, rows
        const CASES: &[(&str, usize, u32, u32)] = &[
            ("empty is one row", 0, 80, 1),
            ("short", 10, 80, 1),
            ("exactly a row", 80, 80, 1),
            ("one over", 81, 80, 2),
            ("long", 400, 80, 5),
            ("a room too narrow for a glyph still takes rows", 3, 0, 3),
        ];
        for (name, glyphs, columns, want) in CASES {
            assert_eq!(rows_of(*glyphs, *columns), *want, "{name}");
        }
    }

    #[test]
    fn a_room_holds_the_glyphs_its_width_has_after_the_gutter() {
        // name, width, glyphs
        const CASES: &[(&str, f32, u32)] = &[
            ("a window", 900.0, 110),
            ("narrower than the gutter", 40.0, 1),
        ];
        for (name, width, want) in CASES {
            assert_eq!(columns(*width), *want, "{name}");
        }
    }

    #[test]
    fn a_page_holds_fewer_lines_when_long_ones_wrap() {
        let lines = [line(10), line(500), line(10), line(10), line(10)];
        // name, rows in the room, wrap, lines that fit (900 px: 110 glyphs a row, 500 takes 5)
        const CASES: &[(&str, u32, Wrap, u32)] = &[
            ("no wrap is one row a line", 3, Wrap::Off, 3),
            ("wrap counts the long line's rows", 6, Wrap::On, 2),
            ("wrap with room for all", 20, Wrap::On, 5),
            ("a long line that fills the room alone", 2, Wrap::On, 1),
            ("a room of no rows still holds one line", 0, Wrap::On, 1),
        ];
        for (name, rows, wrap, want) in CASES {
            assert_eq!(lines_per_page(&lines, *rows, 900.0, *wrap), *want, "{name}");
        }
    }
}
