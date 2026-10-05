//! Text, code, Markdown, tables and JSON: the text stage's view. A file is opened once on a
//! worker (`doc`); the lines on screen are a window a worker read and highlighted, and a
//! Markdown file's page is a sealed frame (`frame`).

mod bar;
mod doc;
mod find;
mod frame;
mod view;
mod wrap;

pub use doc::{LineWindow, TextDoc};
pub use find::FoundHits;
pub(crate) use find::top_for;

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{Job, OpenError, OpenLink};
use crate::{
    Command, LineTotal, LoadFlow, PageLines, PanelTab, PanelTabs, Stage, StageCommand, StageFamily,
    StageIn, StageParams, TextExtent, TextIn, TextParams, TextStage, TextViews, Ticket, TypedText,
};
use anyview_core::{Facts, FormatKind, LineIndex, Resume, Sniffed, Source};
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
    const FLOW: LoadFlow = LoadFlow::PeekThenOpen;
    type Doc = TextDoc;

    fn first_frame(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenLink,
    ) -> Result<Option<TextDoc>, OpenError> {
        doc::first_frame(src, sniffed, link)
    }

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
            text: TextParams {
                views,
                extent: TextExtent {
                    lines: LineTotal(doc.line_count().0),
                    page: PageLines(0),
                },
            },
            ..StageParams::default()
        }
    }

    fn params_seen(
        doc: &TextDoc,
        stage: &Stage,
        area: Option<Area>,
        lines: Option<&LineWindow>,
    ) -> StageParams {
        let params = Self::params(doc, stage, area);
        let (Stage::Text(text), Some(area)) = (stage, area) else {
            return params;
        };
        let place = view::place_of_text(text);
        let rows = view::rows_of(area.size.height.0);
        let from = lines.map_or(&[][..], |window| window.from(place.line));
        let page = wrap::lines_per_page(from, rows, area.size.width.0, place.wrap);
        StageParams {
            text: TextParams {
                extent: TextExtent {
                    page: PageLines(page),
                    ..params.text.extent
                },
                ..params.text
            },
            ..params
        }
    }

    fn arrived(_doc: &TextDoc, stage: &Stage, _left_at: &Resume) -> Vec<StageIn> {
        match stage {
            Stage::Text(TextStage::Finding { query, .. }) => {
                vec![StageIn::Text(TextIn::Find(query.clone()))]
            }
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Table(_)
            | Stage::Tree(_)
            | Stage::Text(TextStage::Reading { .. }) => Vec::new(),
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

    fn search(doc: &Arc<TextDoc>, ticket: Ticket, query: &TypedText) -> Option<Job> {
        Some(Job::Search {
            ticket,
            doc: Arc::clone(doc),
            query: query.clone(),
        })
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
