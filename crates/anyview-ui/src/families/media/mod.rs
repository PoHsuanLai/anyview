//! Video and audio: the media stage's view. The player is the binary's (`io::media`); the window
//! holds a line to it, sends it the stage machine's commands and shows what it reports. A video
//! is a `TextureLayer` of the picture the player draws; an audio file is its cover, or a card of
//! an accent colour and a large title.

mod audio;
mod capsule;
mod doc;
mod live;
mod panel;
mod shelf;
mod view;

pub(crate) use capsule::{level_to_volume, place_to_time};
pub use doc::MediaDoc;
pub use live::{MediaLive, MediaPlace, TrimMarks};
pub use shelf::{MediaShelf, use_media_shelf};

use crate::families::view::{Area, Held, Leaving, StageCx, StageView};
use crate::io::{MediaLine, OpenError, OpenLink};
use crate::{
    Command, MediaIn, PanelTab, PanelTabs, Stage, StageFamily, StageIn, StageParams, Ticket,
};
use anyview_core::{Facts, Resume, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use std::sync::Arc;

/// Video and audio.
#[derive(Debug, Clone, Copy)]
pub struct MediaStageView;

impl StageView for MediaStageView {
    const FAMILY: StageFamily = StageFamily::Media;
    /// A player keeps playing while its document lives, so the file left behind is not held.
    const LEAVING: Leaving = Leaving::Release;
    type Doc = MediaDoc;

    fn open(
        ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<MediaDoc, OpenError> {
        doc::open(ticket, src, sniffed, link)
    }

    fn facts(doc: &MediaDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &MediaDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info, PanelTab::Tracks, PanelTab::Contents])
    }

    fn params(_doc: &MediaDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        StageParams::default()
    }

    fn arrived(_doc: &MediaDoc, stage: &Stage, left_at: &Resume) -> Vec<StageIn> {
        match (stage, left_at) {
            (
                Stage::Media(_),
                Resume::Media {
                    at,
                    volume,
                    audio,
                    subtitles,
                },
            ) => vec![StageIn::Media(MediaIn::Restore {
                at: *at,
                volume: *volume,
                audio: *audio,
                subtitles: *subtitles,
            })],
            (
                Stage::NoStage
                | Stage::Raster(_)
                | Stage::Pdf(_)
                | Stage::Media(_)
                | Stage::Text(_),
                Resume::Raster { .. }
                | Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Nothing,
            ) => Vec::new(),
        }
    }

    fn stage(doc: &Arc<MediaDoc>, cx: &StageCx) -> Element {
        rsx! { view::MediaContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &MediaDoc, cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
        capsule::slots(doc, cx)
    }

    fn panel(doc: &Arc<MediaDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        panel::body(doc, tab, cx)
    }

    fn line(doc: &MediaDoc) -> Option<Arc<dyn MediaLine>> {
        Some(Arc::clone(doc.line()))
    }
}
