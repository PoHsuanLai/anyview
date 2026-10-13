//! Between the buffer's byte offsets and the surface's positions. A line is a node named by its
//! line number, and a position in it is a byte column into its text as drawn: a tab drawn as
//! spaces (`tabs`) and the preedit included while the IME composes at the caret.

use crate::families::text::tabs;
use anyview_text::Session;
use ds::host::position::{TextPosition, TextRange};

/// The text of `line`, as the buffer holds it.
fn text_of(session: &Session, line: usize) -> String {
    session
        .buffer()
        .lines(line, 1)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The column as drawn of the byte column `col` in `line`.
pub(super) fn drawn_col(session: &Session, line: usize, col: usize) -> usize {
    tabs::drawn_offset(&text_of(session, line), col)
}

/// The text node of `line`.
pub(super) fn node_of(line: usize) -> String {
    line.to_string()
}

/// Where the IME's preedit sits in its line's drawn text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Shown {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// The preedit's place, while one is up.
pub(super) fn shown(session: &Session) -> Option<Shown> {
    let preedit = session.preedit()?;
    let (line, col) = session.place(session.caret());
    Some(Shown {
        line,
        col,
        len: preedit.text.len(),
    })
}

/// The drawn position of `offset`: a column of the line as drawn, past the preedit when the
/// offset is after the caret on its line.
pub(super) fn position_of(session: &Session, offset: usize) -> TextPosition {
    let (line, col) = session.place(offset);
    let past = match shown(session) {
        Some(at) if at.line == line && col > at.col => at.len,
        Some(_) | None => 0,
    };
    TextPosition::new(node_of(line), drawn_col(session, line, col) + past)
}

/// The caret as drawn: inside the preedit where the IME's own cursor is.
pub(super) fn caret_position(session: &Session) -> TextPosition {
    let (line, col) = session.place(session.caret());
    let inside = session.preedit().map_or(0, |preedit| {
        preedit
            .cursor
            .as_ref()
            .map_or(preedit.text.len(), |cursor| cursor.end)
    });
    TextPosition::new(node_of(line), drawn_col(session, line, col) + inside)
}

/// The buffer offset of a drawn position; a position in the preedit is the caret.
pub(super) fn offset_at(session: &Session, at: &TextPosition) -> Option<usize> {
    let line: usize = at.node.0.parse().ok()?;
    let text = text_of(session, line);
    let mut col = at.offset.0;
    if let Some(preedit) = shown(session).filter(|p| p.line == line) {
        let at_preedit = tabs::drawn_offset(&text, preedit.col);
        col = if col >= at_preedit + preedit.len {
            col - preedit.len
        } else {
            col.min(at_preedit)
        };
    }
    Some(session.offset_of(line, tabs::real_offset(&text, col)))
}

/// The selection as a range of drawn positions, cut to the lines `window` holds (offsets), or
/// `None` when nothing of it shows.
pub(super) fn range_in(session: &Session, window: std::ops::Range<usize>) -> Option<TextRange> {
    let selection = session.selection();
    let range = selection.range();
    if range.is_empty() || range.end <= window.start || range.start >= window.end {
        return None;
    }
    let clamp = |offset: usize| offset.clamp(window.start, window.end);
    Some(TextRange {
        anchor: position_of(session, clamp(selection.anchor)),
        focus: position_of(session, clamp(selection.focus)),
    })
}
