//! The peek of an office or iWork document: its title, author and size from the package, and the
//! picture the document carries of itself when it has one. Nothing is rendered.

use crate::body::Body;
use crate::described::Described;
use crate::error::PeekError;
use crate::frames::cover_picture;
use anyview_archive::{OfficeLook, ThumbnailCodec, office_look_until};
use anyview_core::work::Interrupt;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Input, Peek, PeekBudget, Sniffed,
};
use anyview_image::ImagePeek;
use std::sync::Arc;

/// What a peek of an office document holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeLooked {
    /// The file's type in words.
    pub described: Described,
    /// What the package says, without the picture (that is `picture`, already decoded).
    pub look: OfficeLook,
    /// The document's own picture, reduced to the budget.
    pub picture: Option<Arc<ImagePeek>>,
}

/// The peek of the kind `Office`.
#[derive(Debug, Clone, Copy)]
pub struct OfficePeek;

impl Peek for OfficePeek {
    const KIND: FormatKind = FormatKind::Office;
    type Peeked = OfficeLooked;
    type Error = PeekError;

    fn peek(
        src: &Input,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<OfficeLooked, PeekError> {
        let described = Described::of(src, sniffed);
        let FormatDetail::Office(format) = sniffed.detail() else {
            return Ok(OfficeLooked {
                described,
                look: OfficeLook::default(),
                picture: None,
            });
        };
        let deadline = budget.deadline_from_now();
        let mut look = office_look_until(src, *format, &deadline)?;
        // Decoding the document's own picture is the slow step: it is skipped once time is up, and
        // the card is the facts.
        let picture = look
            .thumbnail
            .take()
            .filter(|_| !deadline.interrupted())
            .and_then(|thumbnail| {
                let name = match thumbnail.codec {
                    ThumbnailCodec::Png => "thumbnail.png",
                    ThumbnailCodec::Jpeg => "thumbnail.jpg",
                };
                cover_picture(&thumbnail.bytes, name, budget)
            });
        Ok(OfficeLooked {
            described,
            look,
            picture: picture.map(Arc::new),
        })
    }

    fn facts(peeked: &OfficeLooked) -> Facts {
        peeked.look.facts().rows().iter().fold(
            Facts::empty().with(
                FactLabel::Kind,
                FactValue::text(peeked.described.kind.clone()),
            ),
            |facts, row| facts.with(row.label, row.value.clone()),
        )
    }
}

impl From<OfficeLooked> for Body {
    fn from(peeked: OfficeLooked) -> Self {
        match peeked.picture {
            Some(picture) => Body::Picture(picture),
            None => Body::FactsOnly(peeked.described),
        }
    }
}
