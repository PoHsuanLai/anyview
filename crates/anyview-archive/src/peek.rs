//! The peek of an archive: its first entries and how many there are.

use crate::container::kind_name;
use crate::entry::{EntryLimit, Holds, Listing};
use crate::error::ArchiveError;
use crate::list::list;
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, Sniffed, Source,
};

/// The entries a peek keeps; the pane shows about this many.
pub const PEEK_ENTRIES: EntryLimit = EntryLimit(40);

/// What a peek of an archive holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivePeeked {
    /// The start of the index.
    pub listing: Listing,
}

/// The peek of the kind `Archive`.
#[derive(Debug, Clone, Copy)]
pub struct ArchivePeek;

impl Peek for ArchivePeek {
    const KIND: FormatKind = FormatKind::Archive;
    type Peeked = ArchivePeeked;
    type Error = ArchiveError;

    fn peek(
        src: &Source,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<ArchivePeeked, ArchiveError> {
        let FormatDetail::Archive(format) = sniffed.detail() else {
            return Err(ArchiveError::WrongKind {
                kind: sniffed.kind(),
            });
        };
        let listing = list(src.path(), *format, PEEK_ENTRIES, budget.bytes)?;
        Ok(ArchivePeeked { listing })
    }

    fn facts(peeked: &ArchivePeeked) -> Facts {
        let listing = &peeked.listing;
        let kind = kind_name(listing.format, listing.holds);
        let facts = Facts::empty().with(FactLabel::Kind, FactValue::text(kind));
        match listing.holds {
            Holds::Entries => facts.with(
                FactLabel::Entries,
                FactValue::text(format!(
                    "{}, {} unpacked",
                    listing.count.text(),
                    FactValue::size(listing.unpacked).as_str()
                )),
            ),
            Holds::OneFile => facts,
        }
    }
}
