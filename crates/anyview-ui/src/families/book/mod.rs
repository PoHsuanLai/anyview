//! Books and comics: the book stage's view. A book is opened once on a worker (`doc`); the section
//! on screen is a page a worker unpacked and sealed (`Job::Section`), shown in a sealed frame
//! (`frame`, `view`), and an EPUB's contents fill the side panel (`panel`).

mod doc;
mod frame;
mod panel;
mod view;

pub use doc::{BookDoc, Layout, SectionPage};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{Job, OpenError, OpenLink};
use crate::{
    BookParams, Command, PanelTab, PanelTabs, Stage, StageCommand, StageFamily, StageIn,
    StageParams, Ticket,
};
use anyview_core::{Facts, Resume, SectionIndex, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::Icon;
use std::sync::Arc;

/// EPUB books and CBZ comics.
#[derive(Debug, Clone, Copy)]
pub struct BookStageView;

impl StageView for BookStageView {
    const FAMILY: StageFamily = StageFamily::Book;
    type Doc = BookDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<BookDoc, OpenError> {
        doc::open(src, sniffed, link)
    }

    fn facts(doc: &BookDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(doc: &BookDoc) -> PanelTabs {
        match doc.contents().is_empty() {
            true => PanelTabs::of(&[PanelTab::Info]),
            false => PanelTabs::of(&[PanelTab::Contents, PanelTab::Info]),
        }
    }

    fn params(doc: &BookDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        StageParams {
            book: BookParams {
                sections: doc.sections(),
            },
            ..StageParams::default()
        }
    }

    fn arrived(_doc: &BookDoc, stage: &Stage, left_at: &Resume) -> Vec<StageIn> {
        stage.restoring(left_at).into_iter().collect()
    }

    fn stage(doc: &Arc<BookDoc>, cx: &StageCx) -> Element {
        rsx! { view::BookContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &BookDoc, cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
        let here = view::section_of(&cx.stage).map_or(0, |section| section.0);
        let place = format!("{} / {}", here + 1, doc.sections().get());
        let stage = |command| Command::Stage(command);
        vec![
            CapsuleSlot::button(
                stage(StageCommand::PreviousPage),
                "Previous",
                Icon::ChevronLeft,
            ),
            CapsuleSlot::Readout(place),
            CapsuleSlot::button(stage(StageCommand::NextPage), "Next", Icon::ChevronRight),
        ]
    }

    fn panel(doc: &Arc<BookDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        match tab {
            PanelTab::Contents => {
                Some(rsx! { panel::Contents { doc: Held(Arc::clone(doc)), cx: cx.clone() } })
            }
            PanelTab::Thumbnails | PanelTab::Info | PanelTab::Tracks => None,
        }
    }

    fn section(doc: &Arc<BookDoc>, ticket: Ticket, section: SectionIndex) -> Option<Job> {
        Some(Job::Section {
            ticket,
            doc: Arc::clone(doc),
            section,
        })
    }
}
