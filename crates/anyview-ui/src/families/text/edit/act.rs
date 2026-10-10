//! What the editor does with the surface's input: edits the session, asks the surface's geometry
//! for the moves that follow what is drawn (a wrapped row up, the start of a row), keeps the
//! caret's line in the room, and tells the window when the text changed.

use super::command::{Command, command_of};
use super::place::{caret_position, offset_at, position_of};
use crate::Changes;
use anyview_text::{Motion, Revision, Session};
use dioxus::prelude::*;
use ds::edit::handle::EditHandle;
use ds::edit::input::{Composition, EditInput, KeyInput};
use ds::edit::pointer::{EditPointer, Extend};
use ds::host::captured::PointerPhase;
use ds::host::pasted::Pasted;
use ds::host::position::TextPosition;
use ds::prelude::{Point, Px, Rect};

/// The editor's state and its way out, all of it `Copy`: a handler takes a copy.
#[derive(Clone, Copy)]
pub(super) struct Act {
    pub session: Signal<Option<Session>>,
    /// The x a run of Up and Down keeps, in the surface's coordinates.
    pub goal_x: Signal<Option<f32>>,
    /// Whether the caret, at a wrapped row's break, sits at the end of the row above it. The
    /// surface answers a position only for the row it starts, so this is ours.
    pub upstream: Signal<bool>,
    pub handle: EditHandle,
    /// The line at the top of the room.
    pub first: u32,
    /// How many lines the room holds.
    pub rows: u32,
    /// The room should show this line at its top.
    pub scroll: EventHandler<u32>,
    /// The text changed, and has unsaved changes or has not.
    pub changed: EventHandler<Changes>,
}

impl Act {
    /// `edit` on the session, if there is one.
    fn with<R>(&self, edit: impl FnOnce(&mut Session) -> R) -> Option<R> {
        let mut signal = self.session;
        let mut held = signal.write();
        held.as_mut().map(edit)
    }

    /// `read` of the session, if there is one.
    fn read<R>(&self, read: impl FnOnce(&Session) -> R) -> Option<R> {
        self.session.peek().as_ref().map(read)
    }

    fn revision(&self) -> Option<Revision> {
        self.read(Session::revision)
    }

    /// One piece of the surface's input.
    pub(super) fn input(mut self, input: EditInput) {
        let before = self.revision();
        self.upstream.set(false);
        match input {
            EditInput::Text(text) => {
                self.goal_x.set(None);
                self.with(|session| session.type_text(&text));
            }
            EditInput::Key(key) => self.key(&key),
            EditInput::Composition(step) => {
                self.goal_x.set(None);
                self.compose(step);
            }
            EditInput::Paste(Pasted::Text(text) | Pasted::Html { text, .. }) => {
                self.goal_x.set(None);
                self.with(|session| session.paste(&text));
            }
            EditInput::Cut => {
                self.goal_x.set(None);
                if let Some(Some(text)) = self.with(Session::cut) {
                    // A clipboard that cannot be written leaves the text cut, as any editor does.
                    let _unwritten = ds_blitz::clipboard::write_text(&text);
                }
            }
            EditInput::Copy => {
                if let Some(Some(text)) = self.read(Session::selected_text) {
                    let _unwritten = ds_blitz::clipboard::write_text(&text);
                }
            }
        }
        self.finish(before);
    }

    /// After an input: the room follows the caret, and the window hears of a change.
    fn finish(self, before: Option<Revision>) {
        self.follow();
        if self.revision() != before
            && let Some(changes) = self.read(changes_of)
        {
            self.changed.call(changes);
        }
    }

    fn compose(self, step: Composition) {
        self.with(|session| match step {
            Composition::Start => {}
            Composition::Update { text, cursor } => {
                session.compose(&text, cursor.map(|c| c.start..c.end));
            }
            Composition::End { text } => session.end_composition(&text),
        });
    }

    fn key(mut self, key: &KeyInput) {
        let Some(command) = command_of(&key.key, key.modifiers) else {
            return;
        };
        let vertical = matches!(
            command,
            Command::Visual {
                by: Motion::Up | Motion::Down,
                ..
            }
        );
        if !vertical {
            self.goal_x.set(None);
        }
        if let Command::Visual { by, extend } = command {
            return self.visual(by, extend);
        }
        let rows = self.rows;
        self.with(|session| match command {
            Command::Move { by, extend } => session.move_by(by, extend),
            Command::Visual { .. } => {}
            Command::Enter => session.enter(),
            Command::Tab => session.type_text("\t"),
            Command::Backspace => session.backspace(),
            Command::Delete => session.delete_forward(),
            Command::SelectAll => session.select_all(),
            Command::Undo => session.undo(),
            Command::Redo => session.redo(),
            Command::Page(sign) => {
                let (line, col) = session.place(session.caret());
                let to =
                    usize::try_from(i64::from(sign) * i64::from(rows) + line as i64).unwrap_or(0);
                let at = session.offset_of(to, col);
                session.set_caret(at);
            }
        });
    }

    /// Move by what is drawn. The surface answers where a point is; when it cannot (the target
    /// row is not drawn, the document is busy), the move is the text's own.
    fn visual(mut self, by: Motion, extend: bool) {
        let target = self.aimed(by);
        if let (Motion::LineEnd, Some(offset)) = (by, target) {
            let ends_a_row = self.breaks_row(offset);
            self.upstream.set(ends_a_row);
        }
        self.with(|session| match target {
            Some(offset) if extend => session.extend_to(offset),
            Some(offset) => session.set_caret(offset),
            None => session.move_by(by, extend),
        });
    }

    fn aimed(mut self, by: Motion) -> Option<usize> {
        let (caret, from) = self.read(|session| (caret_position(session), session.caret()))?;
        let rect = self.handle.caret_rect(&caret).found()?;
        if by == Motion::LineEnd {
            return self.row_end(&caret, rect.origin.y.0);
        }
        let bounds = self.handle.bounds().found()?;
        let mid = rect.origin.y.0 + rect.size.height.0 / 2.0;
        let x = self.goal_x.peek().unwrap_or(rect.origin.x.0);
        let (at, keeps) = match by {
            Motion::Up => (point(x, mid - rect.size.height.0), true),
            Motion::Down => (point(x, mid + rect.size.height.0), true),
            Motion::LineStart => (point(self.row_start(&caret)? + 0.5, mid), false),
            Motion::LineEnd => (point(bounds.right().0 - 0.5, mid), false),
            Motion::Left
            | Motion::Right
            | Motion::WordLeft
            | Motion::WordRight
            | Motion::DocStart
            | Motion::DocEnd => return None,
        };
        let found = self.handle.hit_test(at).found()?;
        let offset = self.read(|session| offset_at(session, &found))??;
        let right_way = match by {
            Motion::Up => offset < from,
            Motion::Down => offset > from,
            Motion::LineStart | Motion::LineEnd => true,
            Motion::Left
            | Motion::Right
            | Motion::WordLeft
            | Motion::WordRight
            | Motion::DocStart
            | Motion::DocEnd => false,
        };
        if keeps {
            self.goal_x.set(Some(x));
        }
        right_way.then_some(offset)
    }

    /// Where the drawn row the caret is on ends: the first place on its line drawn on a row below
    /// (the wrap point), else the end of the line. Found by asking the surface where the line's
    /// columns are drawn, so it does not depend on where the row's right edge is.
    fn row_end(&self, caret: &TextPosition, row_y: f32) -> Option<usize> {
        let (line, col, len, start) = self.read(|session| {
            let (line, col) = session.place(session.caret());
            let range = session.buffer().line_range(line);
            (line, col, range.len(), range.start)
        })?;
        let below = |at: usize| -> Option<bool> {
            let snapped = self.read(|session| session.offset_of(line, at) - start)?;
            let position = TextPosition::new(caret.node.0.clone(), snapped);
            let rect = self.handle.caret_rect(&position).found()?;
            Some(rect.origin.y.0 > row_y + 1.0)
        };
        let (mut lo, mut hi) = (col, len);
        if !below(len)? {
            return self.read(|session| session.offset_of(line, len));
        }
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if below(mid)? {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        self.read(|session| session.offset_of(line, hi))
    }

    /// Whether `offset` is where a wrapped row breaks: drawn at the start of the row below, though
    /// it is also the end of the row above.
    fn breaks_row(&self, offset: usize) -> bool {
        let Some((here, before)) = self.read(|session| {
            (
                position_of(session, offset),
                position_of(session, session.grapheme_before(offset)),
            )
        }) else {
            return false;
        };
        if before == here {
            return false;
        }
        let (Some(a), Some(b)) = (
            self.handle.caret_rect(&here).found(),
            self.handle.caret_rect(&before).found(),
        ) else {
            return false;
        };
        a.origin.y.0 > b.origin.y.0 + 1.0
    }

    /// The x where a drawn row starts: where its line's column 0 is.
    fn row_start(&self, caret: &TextPosition) -> Option<f32> {
        let start = TextPosition::new(caret.node.0.clone(), 0);
        Some(self.handle.caret_rect(&start).found()?.origin.x.0)
    }

    /// A press, drag or release of the pointer over the surface.
    pub(super) fn pointer(mut self, pointer: EditPointer) {
        self.upstream.set(false);
        let Some(at) = pointer.position.as_ref() else {
            return;
        };
        let Some(Some(offset)) = self.read(|session| offset_at(session, at)) else {
            return;
        };
        self.with(
            |session| match (pointer.phase, pointer.extend, pointer.clicks.0) {
                (PointerPhase::Press, Extend::FromAnchor, _) => session.extend_to(offset),
                (PointerPhase::Press, Extend::Fresh, 2) => session.select_word(offset),
                (PointerPhase::Press, Extend::Fresh, 3) => session.select_line(offset),
                (PointerPhase::Press, Extend::Fresh, _) => session.set_caret(offset),
                (PointerPhase::Drag, _, 1) => session.extend_to(offset),
                (PointerPhase::Drag | PointerPhase::Release, _, _) => {}
            },
        );
        self.goal_x.set(None);
    }

    /// Scroll so the caret's line is in the room.
    fn follow(self) {
        let Some(line) = self.read(|session| session.place(session.caret()).0) else {
            return;
        };
        let line = u32::try_from(line).unwrap_or(u32::MAX);
        if line < self.first {
            self.scroll.call(line);
        } else if line >= self.first.saturating_add(self.rows) {
            self.scroll.call(line + 1 - self.rows.max(1));
        }
    }

    /// Where the IME's candidate window goes: the caret's box in window coordinates.
    pub(super) fn ime_area(&self, caret: Rect) -> Option<Rect> {
        let bounds = self.handle.bounds().found()?;
        Some(Rect {
            origin: Point {
                x: Px(bounds.origin.x.0 + caret.origin.x.0),
                y: Px(bounds.origin.y.0 + caret.origin.y.0),
            },
            size: caret.size,
        })
    }
}

/// Whether the session's text has changes that are not on disk.
pub(super) fn changes_of(session: &Session) -> Changes {
    if session.is_modified() {
        Changes::Unsaved
    } else {
        Changes::Saved
    }
}

fn point(x: f32, y: f32) -> Point {
    Point { x: Px(x), y: Px(y) }
}
