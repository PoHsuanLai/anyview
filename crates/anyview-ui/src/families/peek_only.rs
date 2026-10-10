//! A file the viewer has no stage for: its facts. It is the stage of every kind
//! the full tier does not cover yet (the registry maps them here), and it is a real view, not a
//! stub: a person sees what the file is and hands it to the program that can show it.

use crate::families::InfoCard;
use crate::families::view::{Area, StageCx, StageView};
use crate::io::{OpenError, OpenPort, Readable};
use crate::{Command, PanelTab, PanelTabs, Stage, StageFamily, StageParams, Ticket};
use anyview_archive::{OfficeLook, ThumbnailCodec, office_look};
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FileAction, FileHead, FileName, FormatDetail, FormatKind,
    PeekBudget, PixelArea, RasterTarget, SniffStep, Sniffed, Source, sniff,
};
use anyview_fs::OnDisk;
use anyview_image::{Decoded, decode_bytes, encode};
use anyview_peek::{Body, peek};
use dioxus::prelude::*;
use ds::components::content::image_source::ImageSource;
use ds::components::controls::button::Button;
use ds_core::word::Word;
use std::sync::Arc;
use std::time::Duration;

/// What a card may spend: a listing of an archive reads inside it, and nothing else here is big.
const CARD_BUDGET: PeekBudget = PeekBudget {
    bytes: ByteLen(16 << 20),
    pixels: PixelArea(1_000_000),
    time: Duration::from_secs(2),
};

/// What a file shows when the viewer has no stage for it.
#[derive(Debug, Clone, PartialEq)]
pub struct PeekOnlyDoc {
    /// What the file is called.
    pub name: String,
    /// What the file is.
    pub kind: FormatKind,
    /// Its rows.
    pub facts: Facts,
    /// The picture an office document carries of itself, ready to draw.
    pub thumbnail: Option<ImageSource>,
    /// What an archive holds, one line an entry.
    pub listing: Vec<String>,
    /// Whether the contents could be read.
    pub readable: Readable,
}

/// The picture of an office document as a PNG `data:` source, or `None` when it has none or it
/// will not decode: the facts stand without it.
fn thumbnail_of(look: &OfficeLook) -> Option<ImageSource> {
    let thumbnail = look.thumbnail.as_ref()?;
    let name = match thumbnail.codec {
        ThumbnailCodec::Png => "thumbnail.png",
        ThumbnailCodec::Jpeg => "thumbnail.jpg",
    };
    let head = FileHead::new(&thumbnail.bytes[..thumbnail.bytes.len().min(4096)]);
    let SniffStep::Done(sniffed) = sniff(&head, &FileName::new(name).ok()?) else {
        return None;
    };
    let Decoded::Still(picture) = decode_bytes(&thumbnail.bytes, &sniffed).ok()? else {
        return None;
    };
    encode(&picture, RasterTarget::Png)
        .ok()
        .map(|png| ImageSource::png(&png))
}

/// What the light tier makes of a file the viewer has no stage for.
struct Card {
    /// The rows: a friendly kind, the size, and what the format itself says (a font's family, an
    /// archive's entry count, a folder's tally).
    facts: Facts,
    /// The entries of an archive, one line each, in the archive's own order.
    listing: Vec<String>,
    /// Whether the file's contents could be read: it looks damaged when they could not.
    readable: Readable,
}

/// The peek of `src`, a file of `sniffed`'s type, as a card. Blocking. The sniffing is the probe's:
/// `anyview_peek::look` would sniff the file a second time.
fn card_of(src: &Source, sniffed: &Sniffed) -> Card {
    let peeked = peek(src.on_disk(), sniffed, &CARD_BUDGET);
    let (listing, readable) = match &peeked.body {
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
    Card {
        facts: peeked.facts,
        listing,
        readable,
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

/// The line under a file's name: what the viewer does for the kind, in words that never say the
/// viewer is lacking (a mature viewer says what to do instead).
fn description_of(doc: &PeekOnlyDoc) -> String {
    let unreadable = doc.readable == Readable::No;
    match doc.kind {
        FormatKind::Archive if unreadable => {
            "The contents could not be read. The archive may be damaged.".to_owned()
        }
        FormatKind::Archive if doc.listing.is_empty() => {
            "Open it with an app that extracts archives.".to_owned()
        }
        FormatKind::Archive => "What the archive holds is listed below.".to_owned(),
        FormatKind::Folder => "A folder.".to_owned(),
        FormatKind::Font => "Open it with a font app to look at it or install it.".to_owned(),
        FormatKind::Office => "Open it with an office app to read or edit it.".to_owned(),
        FormatKind::Raster
        | FormatKind::Vector
        | FormatKind::Pdf
        | FormatKind::PlainText
        | FormatKind::Markdown
        | FormatKind::Code
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Video
        | FormatKind::Audio
        | FormatKind::Book
        | FormatKind::Other => {
            "The viewer can\u{2019}t show this kind of file. Another app may be able to open it."
                .to_owned()
        }
    }
}

/// Every kind the full tier does not show yet.
#[derive(Debug, Clone, Copy)]
pub struct PeekOnlyStageView;

impl StageView for PeekOnlyStageView {
    const FAMILY: StageFamily = StageFamily::PeekOnly;
    type Doc = PeekOnlyDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        _link: &OpenPort,
    ) -> Result<PeekOnlyDoc, OpenError> {
        let name = src
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned());
        let base = Facts::empty()
            .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
            .with(FactLabel::Size, FactValue::size(src.stamp().len));
        let look = match sniffed.detail() {
            FormatDetail::Office(format) => office_look(src.on_disk(), *format).unwrap_or_default(),
            FormatDetail::None
            | FormatDetail::Raster(_)
            | FormatDetail::Code(_)
            | FormatDetail::Table(_)
            | FormatDetail::Tree(_)
            | FormatDetail::Text(_)
            | FormatDetail::Media(_)
            | FormatDetail::Font(_)
            | FormatDetail::Archive(_)
            | FormatDetail::Book(_) => OfficeLook::default(),
        };
        // The light tier's card says what the launcher's pane says of the file: its friendly kind
        // and what the format holds. A card with no rows leaves the kind and size the viewer knows.
        let card = card_of(src, sniffed);
        let start = if card.facts.rows().is_empty() {
            base
        } else {
            card.facts.clone()
        };
        let facts = look
            .facts()
            .rows()
            .iter()
            .fold(start, |facts, row| facts.with_fact(row.clone()));
        Ok(PeekOnlyDoc {
            name,
            kind: sniffed.kind(),
            facts,
            thumbnail: thumbnail_of(&look),
            listing: card.listing,
            readable: card.readable,
        })
    }

    fn facts(doc: &PeekOnlyDoc) -> Facts {
        doc.facts.clone()
    }

    // The stage is the file's card already: a panel beside it would say the same again.
    fn tabs(_doc: &PeekOnlyDoc) -> PanelTabs {
        PanelTabs::NONE
    }

    fn params(_doc: &PeekOnlyDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        StageParams::default()
    }

    fn stage(doc: &Arc<PeekOnlyDoc>, cx: &StageCx) -> Element {
        let run = cx.run;
        let platform = cx.platform;
        let description = description_of(doc);
        rsx! {
            div { class: "viewer-peek",
                div { class: "viewer-peek-body",
                    InfoCard {
                        name: doc.name.clone(),
                        kind: Some(doc.kind),
                        facts: doc.facts.clone(),
                        thumbnail: doc.thumbnail.clone(),
                        note: Some(description),
                        actions: rsx! {
                            div { class: "viewer-failed-actions",
                                if platform.offers(FileAction::RevealInFolder) {
                                    Button {
                                        label: "Show in Folder",
                                        onclick: move |_| run.call(Command::File(FileAction::RevealInFolder)),
                                    }
                                }
                            }
                        },
                    }
                    if !doc.listing.is_empty() {
                        ul { class: "viewer-peek-listing", aria_label: "Contents",
                            for line in doc.listing.iter() {
                                li { "{line}" }
                            }
                        }
                    }
                }
            }
        }
    }

    fn slots(
        _doc: &PeekOnlyDoc,
        _cx: &StageCx,
    ) -> Vec<ds::components::chrome::capsule::priority::RankedSlot<Command>> {
        Vec::new()
    }

    fn panel(_doc: &Arc<PeekOnlyDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
