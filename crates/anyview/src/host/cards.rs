//! The cards of the files the viewer has no stage for, from the launcher's own light tier: a font,
//! an archive, a folder or an office document shows what its peek shows in the pane.

use anyview_core::{ByteLen, FactValue, PeekBudget, PixelArea, Sniffed, Source};
use anyview_peek::{Body, peek};
use anyview_ui::{FileCard, FileCards, Readable};
use ds::prelude::Word;
use std::time::Duration;

/// What a card may spend: a listing of an archive reads inside it, and nothing else here is big.
const CARD_BUDGET: PeekBudget = PeekBudget {
    bytes: ByteLen(16 << 20),
    pixels: PixelArea(1_000_000),
    time: Duration::from_secs(2),
};

/// `anyview-peek` as the views' source of cards.
#[derive(Debug, Clone, Copy, Default)]
pub struct PeekCards;

impl FileCards for PeekCards {
    fn card(&self, source: &Source, sniffed: &Sniffed) -> Option<FileCard> {
        let peeked = peek(source, sniffed, &CARD_BUDGET);
        let (listing, unreadable) = match &peeked.body {
            Body::Archive(archive) => (
                archive
                    .listing
                    .entries
                    .iter()
                    .map(|entry| line_of(&entry.path, entry.kind.slug(), entry.size))
                    .collect(),
                Readable::Yes,
            ),
            Body::Unavailable(_) => (Vec::new(), Readable::No),
            Body::Picture(_)
            | Body::Page(_)
            | Body::Plain(_)
            | Body::Code(_)
            | Body::Markdown(_)
            | Body::Table(_)
            | Body::Tree(_)
            | Body::Font(_)
            | Body::Folder(_)
            | Body::FactsOnly(_) => (Vec::new(), Readable::Yes),
        };
        Some(FileCard {
            facts: peeked.facts,
            listing,
            unreadable,
        })
    }
}

/// One line of a listing: the entry's path, a slash after a folder's, and the size of a file
/// when it is known. `kind` is the entry kind's slug.
fn line_of(path: &str, kind: &str, size: Option<ByteLen>) -> String {
    match (kind, size) {
        ("directory", _) => format!("{path}/"),
        ("file", Some(size)) => format!("{path}   {}", FactValue::size(size).as_str()),
        _ => path.to_owned(),
    }
}
