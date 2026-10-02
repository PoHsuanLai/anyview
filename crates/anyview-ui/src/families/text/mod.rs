//! Text, code, Markdown, tables and JSON: the text stage's view. A file is opened once on a
//! worker (`doc`); the lines on screen are a window a worker read and highlighted, and a
//! Markdown file's page is a sealed frame (`frame`).

mod doc;
mod frame;
mod view;

pub use doc::{LineWindow, TextDoc};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{Job, OpenError, OpenLink};
use crate::{
    Command, PanelTab, PanelTabs, Stage, StageCommand, StageFamily, StageParams, TextParams,
    TextViews, Ticket,
};
use anyview_core::{Facts, FormatKind, LineIndex, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::prelude::Icon;
use std::sync::Arc;

/// The views a file of `kind` has: Markdown has its page and its source, everything else its
/// source only.
pub(crate) fn views_of(kind: FormatKind) -> TextViews {
    if kind == FormatKind::Markdown {
        TextViews::RenderedAndSource
    } else {
        TextViews::SourceOnly
    }
}

/// Plain text, source code, Markdown, delimited tables and JSON, shown as text.
#[derive(Debug, Clone, Copy)]
pub struct TextStageView;

impl StageView for TextStageView {
    const FAMILY: StageFamily = StageFamily::Text;
    type Doc = TextDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<TextDoc, OpenError> {
        doc::open(src, sniffed, link)
    }

    fn facts(doc: &TextDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(_doc: &TextDoc) -> PanelTabs {
        PanelTabs::of(&[PanelTab::Info])
    }

    fn params(doc: &TextDoc, _stage: &Stage, _area: Option<Area>) -> StageParams {
        let views = match doc.rendered {
            Some(_) => TextViews::RenderedAndSource,
            None => TextViews::SourceOnly,
        };
        StageParams {
            text: TextParams { views },
            ..StageParams::default()
        }
    }

    fn stage(doc: &Arc<TextDoc>, cx: &StageCx) -> Element {
        rsx! { view::TextContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &TextDoc, _cx: &StageCx) -> Vec<CapsuleSlot<Command>> {
        let mut slots = vec![CapsuleSlot::button(
            Command::Stage(StageCommand::ToggleWrap),
            "Wrap lines",
            Icon::Columns,
        )];
        if doc.rendered.is_some() {
            slots.push(CapsuleSlot::button(
                Command::Stage(StageCommand::ToggleSource),
                "Rendered or source",
                Icon::Code,
            ));
        }
        slots
    }

    fn panel(_doc: &Arc<TextDoc>, _tab: PanelTab, _cx: &StageCx) -> Option<Element> {
        None
    }

    fn lines(doc: &Arc<TextDoc>, ticket: Ticket, first: LineIndex, rows: u32) -> Option<Job> {
        Some(Job::Lines {
            ticket,
            doc: Arc::clone(doc),
            first,
            rows,
        })
    }
}
