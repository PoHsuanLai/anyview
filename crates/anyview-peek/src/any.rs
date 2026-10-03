//! The type-erased result of a peek: facts and a body, whichever kind of file it was.

use crate::body::{Body, Light};
use crate::described::Described;
use crate::frames::{NoFrames, VideoFrames, still_peek};
use crate::registry::{KindVisitor, visit};
use crate::when::modified_text;
use anyview_core::{FactLabel, FactValue, Facts, FormatKind, PeekBudget, Sniffed, Source};
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

/// Peeks at `src`, whose type `sniffed` established, inside `budget`: blocking, so run it on a
/// worker. It never fails: a peek that cannot be made comes back as [`Body::Unavailable`] with
/// the reason, and the facts the file can still give (its type, size and date).
pub fn peek(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> AnyPeeked {
    peek_with(src, sniffed, budget, &NoFrames)
}

/// [`peek`], with `frames` as the host's source of a picture for a video that carries no cover. The
/// frame replaces only the facts-only card of a video whose header was read; a failed peek, and
/// every other kind, are as [`peek`] makes them.
pub fn peek_with(
    src: &Source,
    sniffed: &Sniffed,
    budget: &PeekBudget,
    frames: &dyn VideoFrames,
) -> AnyPeeked {
    let mut peeked = peek_kind(src, sniffed, budget);
    if peeked.kind == FormatKind::Video
        && matches!(peeked.body, Body::FactsOnly(_))
        && let Some(picture) = frames.frame(src)
    {
        peeked.body = Body::Picture(Arc::new(still_peek(picture, budget)));
    }
    peeked
}

fn peek_kind(src: &Source, sniffed: &Sniffed, budget: &PeekBudget) -> AnyPeeked {
    visit(
        sniffed.kind(),
        Run {
            src,
            sniffed,
            budget,
        },
    )
}

/// The visitor that runs the peek of whichever kind it is given.
struct Run<'a> {
    src: &'a Source,
    sniffed: &'a Sniffed,
    budget: &'a PeekBudget,
}

impl KindVisitor for Run<'_> {
    type Out = AnyPeeked;

    fn visit<P: Light>(self) -> AnyPeeked {
        let (facts, body) = match P::peek(self.src, self.sniffed, self.budget) {
            Ok(peeked) => (P::facts(&peeked), peeked.into()),
            Err(error) => {
                let error: crate::PeekError = error.into();
                let described = Described::of(self.sniffed);
                let facts = Facts::empty().with(FactLabel::Kind, FactValue::text(described.kind));
                (facts, Body::Unavailable(error.to_string()))
            }
        };
        AnyPeeked {
            kind: P::KIND,
            name: name_of(self.src),
            facts: with_file_facts(facts, self.src),
            body,
        }
    }
}

/// The file's name, else its whole path (a root has no name).
fn name_of(src: &Source) -> String {
    match src.path().file_name() {
        Some(name) => name.as_str().to_owned(),
        None => src.path().as_path().display().to_string(),
    }
}

/// `facts` plus the size and the modification time, unless the peek already gave that row.
fn with_file_facts(facts: Facts, src: &Source) -> Facts {
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
