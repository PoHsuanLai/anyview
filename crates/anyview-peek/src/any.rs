//! The type-erased result of a peek: facts and a body, whichever kind of file it was.

use crate::body::{Body, Light};
use crate::described::Described;
use crate::error::PeekError;
use crate::frames::{NoFrames, VideoFrames, still_peek};
use crate::registry::{KindVisitor, visit};
use crate::when::modified_text;
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FormatKind, Input, PeekBudget, Sniffed, is_regular,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

/// What a peek of any file produced: what to draw, and the rows to list beside it. It is the value
/// a worker sends to the pane, and `Clone + PartialEq + Send` like every `Peek::Peeked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnyPeeked {
    /// The kind of file.
    pub kind: FormatKind,
    /// The file's name, or its path when it has none.
    pub name: String,
    /// The rows the pane lists: the format's own, then the size and the date.
    pub facts: Facts,
    /// What to draw.
    pub body: Body,
}

/// Peeks at `src` (a path: `&FilePath`, `&Source`; or any bytes a host injects), whose type `sniffed` established, inside `budget`: blocking, so run it on a
/// worker. It never fails: a peek that cannot be made comes back as [`Body::Unavailable`] with
/// the reason, and the facts the file can still give (its type, size and date).
pub fn peek(src: impl Into<Input>, sniffed: &Sniffed, budget: &PeekBudget) -> AnyPeeked {
    peek_with(src, sniffed, budget, &NoFrames)
}

/// [`peek`], with `frames` as the host's source of a picture for a video that carries no cover. The
/// frame replaces only the facts-only card of a video whose header was read; a failed peek, and
/// every other kind, are as [`peek`] makes them.
pub fn peek_with(
    src: impl Into<Input>,
    sniffed: &Sniffed,
    budget: &PeekBudget,
    frames: &dyn VideoFrames,
) -> AnyPeeked {
    let src = &src.into();
    let mut peeked = peek_kind(src, sniffed, budget);
    if peeked.kind == FormatKind::Video
        && matches!(peeked.body, Body::FactsOnly(_))
        && let Some(picture) = frames.frame(src)
    {
        peeked.body = Body::Picture(Arc::new(still_peek(picture, budget)));
    }
    peeked
}

/// The peek of the file's kind, behind two guards: the path must be a file the kind's peek may
/// read in full, and a panic in a back end (a decoder fed a hostile file) becomes the same
/// unavailable card as any other failure, so no worker thread dies of one file.
fn peek_kind(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> AnyPeeked {
    if let Err(error) = guard(src, sniffed, budget) {
        return unavailable(src, sniffed, &error.to_string());
    }
    let run = Run {
        src,
        sniffed,
        budget,
    };
    contained(src, sniffed, || visit(sniffed.kind(), run))
}

/// `peek`, with a panic in it turned into the unavailable card.
fn contained(src: &Input, sniffed: &Sniffed, peek: impl FnOnce() -> AnyPeeked) -> AnyPeeked {
    catch_unwind(AssertUnwindSafe(peek))
        .unwrap_or_else(|_| unavailable(src, sniffed, "the preview could not be made"))
}

/// Whether `src` may be peeked at all: it is a regular file (a folder for the folder peek), and
/// for a kind whose peek reads the whole file, one within the budget's bytes. A path is read
/// afresh, not from the stamp, so a file that changed since it was probed is judged as it is; the
/// length is the larger of the stamp's and the bytes' own, so a source that understates its size
/// is held to the budget as well.
fn guard(src: &Input, sniffed: &Sniffed, budget: &PeekBudget) -> Result<(), PeekError> {
    let refused = |kind| PeekError::Unreadable {
        path: src.label(),
        kind,
    };
    let folder = sniffed.kind() == FormatKind::Folder;
    let wanted = match src.path() {
        Some(path) => {
            let meta = std::fs::metadata(path.as_path()).map_err(|e| refused(e.kind()))?;
            if folder {
                meta.is_dir()
            } else {
                is_regular(&meta)
            }
        }
        // Only a path can be a folder.
        None => !folder,
    };
    if !wanted {
        return Err(refused(std::io::ErrorKind::InvalidInput));
    }
    let len = ByteLen(src.stamp().len.0.max(src.bytes().len().0));
    if reads_whole_file(sniffed.kind()) && len > budget.bytes {
        return Err(PeekError::OverBudget {
            len,
            allowed: budget.bytes,
        });
    }
    Ok(())
}

/// Whether the peek of `kind` reads the file whole (a picture, a font, a book's package, an
/// office document's parts), rather than its head or a bounded listing.
fn reads_whole_file(kind: FormatKind) -> bool {
    match kind {
        FormatKind::Raster
        | FormatKind::Vector
        | FormatKind::Font
        | FormatKind::Book
        | FormatKind::Office => true,
        FormatKind::Pdf
        | FormatKind::Video
        | FormatKind::Audio
        | FormatKind::Markdown
        | FormatKind::Code
        | FormatKind::PlainText
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Archive
        | FormatKind::Folder
        | FormatKind::Other => false,
    }
}

/// The card of a file whose peek could not be made, for `reason`: its type, size and date.
fn unavailable(src: &Input, sniffed: &Sniffed, reason: &str) -> AnyPeeked {
    let facts = Facts::empty().with(
        FactLabel::Kind,
        FactValue::text(Described::of(sniffed).kind),
    );
    AnyPeeked {
        kind: sniffed.kind(),
        name: name_of(src),
        facts: with_file_facts(facts, src),
        body: Body::Unavailable(reason.to_owned()),
    }
}

/// The visitor that runs the peek of whichever kind it is given.
struct Run<'a> {
    src: &'a Input,
    sniffed: &'a Sniffed,
    budget: &'a PeekBudget,
}

impl KindVisitor for Run<'_> {
    type Out = AnyPeeked;

    fn visit<P: Light>(self) -> AnyPeeked {
        match P::peek(self.src, self.sniffed, self.budget) {
            Ok(peeked) => AnyPeeked {
                kind: P::KIND,
                name: name_of(self.src),
                facts: with_file_facts(P::facts(&peeked), self.src),
                body: peeked.into(),
            },
            Err(error) => {
                let error: PeekError = error.into();
                let mut card = unavailable(self.src, self.sniffed, &error.to_string());
                card.kind = P::KIND;
                card
            }
        }
    }
}

/// The file's name.
fn name_of(src: &Input) -> String {
    src.name().as_str().to_owned()
}

/// `facts` plus the size and the modification time, unless the peek already gave that row.
fn with_file_facts(facts: Facts, src: &Input) -> Facts {
    let stamp = src.stamp();
    let mut facts = facts;
    if facts.value(FactLabel::Size).is_none() {
        facts = facts.with(FactLabel::Size, FactValue::size(stamp.len));
    }
    if facts.value(FactLabel::Modified).is_none() {
        facts = facts.with(
            FactLabel::Modified,
            FactValue::text(modified_text(stamp.modified)),
        );
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FileHead, FileName, SniffStep, sniff};

    #[test]
    fn a_panic_in_a_peek_is_the_unavailable_card_of_that_file() {
        let name = FileName::new("notes.txt").unwrap_or_else(|e| panic!("{e}"));
        let SniffStep::Done(sniffed) = sniff(&FileHead::new(b"hello"), &name) else {
            panic!("plain text is sniffed at once");
        };
        let src = Input::from((name, b"hello".to_vec()));
        let card = contained(&src, &sniffed, || panic!("a decoder fell over"));
        assert_eq!(card.name, "notes.txt");
        assert!(matches!(card.body, Body::Unavailable(_)), "{:?}", card.body);
    }
}
