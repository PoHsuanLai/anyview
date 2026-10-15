//! What a stage that edits the data of a file (a table's cells, a tree's nodes) holds and how it
//! moves. The edits themselves are held by the window, beside the document, because they are as
//! large as the person's changes; the stage holds only what its inputs and its keys depend on: what
//! is being typed in, and what the window last said of the edits (whether they are saved, how many
//! steps can be taken back or done again). The transitions are shared, so a table and a tree behave
//! the same way.

use super::text::{Changes, Editable};
use crate::edits::Rewind;
use crate::typed::TypedText;

/// How many steps of the edits can be taken back or done again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditDepth(pub u32);

/// A number the window gives the state of the edits, a new one each time they change, so a view
/// that draws from the edits draws again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditRevision(pub u64);

/// Whether rows were added or taken away, which the order a sort gave no longer fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowOrder {
    /// The rows are the file's own.
    #[default]
    AsFile,
    /// Rows were added or taken away.
    Changed,
}

/// What the window says of the edits it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditLog {
    /// Whether they are on disk.
    pub changes: Changes,
    /// How many steps can be taken back.
    pub undo: EditDepth,
    /// How many steps can be done again.
    pub redo: EditDepth,
    /// Whether rows were added or taken away.
    pub rows: RowOrder,
    /// Which state of the edits this is.
    pub revision: EditRevision,
}

impl EditLog {
    /// Nothing edited, nothing to take back or do again.
    pub const NONE: EditLog = EditLog {
        changes: Changes::Saved,
        undo: EditDepth(0),
        redo: EditDepth(0),
        rows: RowOrder::AsFile,
        revision: EditRevision(0),
    };
}

/// The first of what is typed into a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypingSeed {
    /// The value as it is: Return, or a double-click.
    Existing,
    /// This text in place of the value: a key typed on a picked cell.
    Typed(TypedText),
}

/// What is being typed in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypingState {
    /// Nothing.
    #[default]
    Idle,
    /// A value: a cell, or the value of a node.
    Value(TypingSeed),
    /// The key of a node.
    Key,
}

/// Where the cursor goes when what was typed is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterTyping {
    /// Nowhere: the caret left.
    Stay,
    /// Down a row: Return.
    Down,
    /// Right a cell: Tab.
    Right,
}

/// What the stage holds of the data edits.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataEdits {
    /// What the window last said of the edits.
    pub log: EditLog,
    /// What is being typed in.
    pub typing: TypingState,
}

impl DataEdits {
    /// Nothing edited and nothing typed in.
    pub const NONE: DataEdits = DataEdits {
        log: EditLog::NONE,
        typing: TypingState::Idle,
    };
}

/// An input about typing into a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypingIn {
    /// Start typing into the value under the cursor. Heard only when the file can be edited.
    Begin(TypingSeed),
    /// The text typed is taken, and the cursor goes where `AfterTyping` says.
    Commit(TypedText, AfterTyping),
    /// The text typed is let go (Esc).
    Cancel,
}

/// An input about the edits as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditSessionIn {
    /// Take back the last edit (⌘Z).
    Undo,
    /// Do the edit taken back again (⇧⌘Z).
    Redo,
    /// Write the edits into the file (⌘S).
    Save,
    /// Let the edits go (Don't Save).
    Discard,
    /// The window changed the edits, or saved them: this is what they are now.
    Logged(EditLog),
}

/// What the stage asks of the window about the edits as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditSessionOut {
    /// Take back the last edit or do it again.
    Rewind(Rewind),
    /// Write the edits into the file.
    Save,
    /// Let the edits go.
    Discard,
}

/// What a commit of typing amounts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Taken {
    /// Nothing was being typed.
    Nothing,
    /// This text is the new value; the cursor goes on.
    Value(TypedText, AfterTyping),
    /// This text is the new key; the cursor goes on.
    Key(TypedText, AfterTyping),
}

/// The edits after `input`, and what a commit took. A file that cannot be edited starts no typing.
pub(super) fn typing(edits: DataEdits, input: TypingIn, can: Editable) -> (DataEdits, Taken) {
    let DataEdits { log, typing } = edits;
    match (input, typing, can) {
        (TypingIn::Begin(seed), TypingState::Idle, Editable::Yes) => (
            DataEdits {
                log,
                typing: TypingState::Value(seed),
            },
            Taken::Nothing,
        ),
        (TypingIn::Commit(text, then), TypingState::Value(_), _) => (
            DataEdits {
                log,
                typing: TypingState::Idle,
            },
            Taken::Value(text, then),
        ),
        (TypingIn::Commit(text, then), TypingState::Key, _) => (
            DataEdits {
                log,
                typing: TypingState::Idle,
            },
            Taken::Key(text, then),
        ),
        (TypingIn::Cancel, _, _) | (TypingIn::Commit(..), TypingState::Idle, _) => (
            DataEdits {
                log,
                typing: TypingState::Idle,
            },
            Taken::Nothing,
        ),
        (TypingIn::Begin(_), typing, Editable::Yes | Editable::No) => {
            (DataEdits { log, typing }, Taken::Nothing)
        }
    }
}

/// The edits after `input`, and what the window is asked to do.
pub(super) fn session(
    edits: DataEdits,
    input: EditSessionIn,
) -> (DataEdits, Option<EditSessionOut>) {
    let DataEdits { log, typing } = edits;
    match input {
        EditSessionIn::Undo if log.undo.0 > 0 => (
            DataEdits {
                log,
                typing: TypingState::Idle,
            },
            Some(EditSessionOut::Rewind(Rewind::Undo)),
        ),
        EditSessionIn::Redo if log.redo.0 > 0 => (
            DataEdits {
                log,
                typing: TypingState::Idle,
            },
            Some(EditSessionOut::Rewind(Rewind::Redo)),
        ),
        EditSessionIn::Save if log.changes == Changes::Unsaved => {
            (DataEdits { log, typing }, Some(EditSessionOut::Save))
        }
        EditSessionIn::Discard => (
            DataEdits {
                log: EditLog {
                    changes: Changes::Saved,
                    ..log
                },
                typing: TypingState::Idle,
            },
            Some(EditSessionOut::Discard),
        ),
        EditSessionIn::Logged(now) => (DataEdits { log: now, typing }, None),
        EditSessionIn::Undo | EditSessionIn::Redo | EditSessionIn::Save => {
            (DataEdits { log, typing }, None)
        }
    }
}
