//! What the viewer asks of its host to change a file: an edit, going back through the edits, and
//! going back to a version. The host does them (they write the person's file); the viewer only
//! says which.

use anyview_core::{Edit, PageIndex};

/// An edit, and the page of a PDF the person is on when they ask. A turn of a PDF turns that
/// page; a picture has no pages and says the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditRequest {
    /// What to change.
    pub edit: Edit,
    /// The page the person is on.
    pub page: PageIndex,
}

impl EditRequest {
    /// `edit` of a picture.
    pub fn of_picture(edit: Edit) -> Self {
        EditRequest {
            edit,
            page: PageIndex(0),
        }
    }

    /// `edit` asked on `page` of a PDF.
    pub fn on_page(edit: Edit, page: PageIndex) -> Self {
        EditRequest { edit, page }
    }
}

/// A step through the saves of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rewind {
    /// Take back the last edit.
    Undo,
    /// Do again the edit that was taken back.
    Redo,
}

/// What saving an edit of the open file in place costs, as the document says when it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EditOffer {
    /// Nothing is lost: the edit goes ahead without a question.
    #[default]
    Plain,
    /// Something is lost: the person is asked before it goes ahead.
    Asks(EditCaution),
    /// No valid file can be written: the edit is not offered.
    Withheld,
}

/// What an edit that asks would cost, in words for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditCaution {
    /// A picture saved this way loses this.
    Loses(&'static str),
    /// A signed document loses its signature.
    Signed,
}

/// How the host's save of the edited text ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveEnd {
    /// The text is in the file, and the original is kept.
    Written,
    /// Nothing was written and the file is as it was.
    Refused,
}
