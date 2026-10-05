//! A file the viewer has no stage for: its facts and Open With…. It is the stage of every kind
//! the full tier does not cover yet (the registry maps them here), and it is a real view, not a
//! stub: a person sees what the file is and hands it to the program that can show it.

use crate::families::view::{Area, StageCx, StageView};
use crate::io::{OpenError, OpenLink, Readable};
use crate::{Command, PanelTab, PanelTabs, Stage, StageFamily, StageParams, Ticket};
use anyview_archive::{OfficeLook, ThumbnailCodec, office_look};
use anyview_core::{
    FactLabel, FactValue, Facts, FileAction, FileHead, FileName, FormatDetail, FormatKind,
    RasterTarget, SniffStep, Sniffed, Source, sniff,
};
use anyview_image::{Decoded, decode_bytes, encode};
use dioxus::prelude::*;
use ds::components::content::image_source::ImageSource;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::button::Button;
use ds::components::fields::fact_list::{Fact, FactList};
use ds::components::overlays::empty_state::EmptyState;
use ds::prelude::Icon;
use ds_core::word::Word;
use std::sync::Arc;

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
        link: &OpenLink,
    ) -> Result<PeekOnlyDoc, OpenError> {
        let name = src
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned());
        let base = Facts::empty()
            .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
            .with(FactLabel::Size, FactValue::size(src.stamp().len));
        let look = match sniffed.detail() {
            FormatDetail::Office(format) => office_look(src.path(), *format).unwrap_or_default(),
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
        // The host's card says what the launcher's pane says of the file: its friendly kind and
        // what the format holds. Without one the viewer lists the kind and size it knows.
        let card = link.cards.card(src, sniffed).unwrap_or_default();
        let start = if card.facts.rows().is_empty() {
            base
        } else {
            card.facts.clone()
        };
        let facts = look
            .facts()
            .rows()
            .iter()
            .fold(start, |facts, row| facts.with(row.label, row.value.clone()));
        Ok(PeekOnlyDoc {
            name,
            kind: sniffed.kind(),
            facts,
            thumbnail: thumbnail_of(&look),
            listing: card.listing,
            readable: card.unreadable,
        })
    }

    fn facts(doc: &PeekOnlyDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &PeekOnlyDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info])
    }

    fn params(_doc: &PeekOnlyDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        StageParams::default()
    }

    fn stage(doc: &Arc<PeekOnlyDoc>, cx: &StageCx) -> Element {
        let run = cx.run;
        let facts: Vec<Fact> = doc
            .facts
            .rows()
            .iter()
            .map(|row| Fact::new(row.label.label(), row.value.as_str()))
            .collect();
        let description = description_of(doc);
        rsx! {
            div { class: "viewer-peek",
                div { class: "viewer-peek-body",
                    if let Some(thumbnail) = doc.thumbnail.clone() {
                        img { class: "viewer-peek-thumbnail", alt: "First page", src: thumbnail.0 }
                    }
                    EmptyState {
                        icon: Icon::File,
                        title: doc.name.clone(),
                        description: Some(TextLine::from(description)),
                        action: rsx! {
                            div { class: "viewer-failed-actions",
                                Button {
                                    label: "Open With\u{2026}",
                                    onclick: move |_| run.call(Command::File(FileAction::OpenWith)),
                                }
                                Button {
                                    label: "Show in Folder",
                                    onclick: move |_| run.call(Command::File(FileAction::RevealInFolder)),
                                }
                            }
                        },
                    }
                    div { class: "viewer-peek-facts", FactList { facts } }
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
    ) -> Vec<ds::components::chrome::capsule::model::CapsuleSlot<Command>> {
        Vec::new()
    }

    fn panel(_doc: &Arc<PeekOnlyDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }
}
