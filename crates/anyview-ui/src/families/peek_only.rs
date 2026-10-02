//! A file the viewer has no stage for: its facts and Open With…. It is the stage of every kind
//! the full tier does not cover yet (the registry maps them here), and it is a real view, not a
//! stub: a person sees what the file is and hands it to the program that can show it.

use crate::families::view::{Area, StageCx, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{Command, PanelTab, PanelTabs, Stage, StageFamily, StageParams, Ticket};
use anyview_core::{FactLabel, FactValue, Facts, FileAction, FormatKind, Sniffed, Source};
use dioxus::prelude::*;
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
        _link: &OpenLink,
    ) -> Result<PeekOnlyDoc, OpenError> {
        let name = src
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned());
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text(sniffed.mime().as_str()))
            .with(FactLabel::Size, FactValue::size(src.stamp().len));
        Ok(PeekOnlyDoc {
            name,
            kind: sniffed.kind(),
            facts,
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
        rsx! {
            div { class: "viewer-peek",
                div { class: "viewer-peek-body",
                EmptyState {
                    icon: Icon::File,
                    title: doc.name.clone(),
                    description: Some(TextLine::from(format!("The viewer cannot show {} files yet.", doc.kind.label().to_lowercase()))),
                    action: rsx! {
                        Button {
                            label: "Open With…",
                            onclick: move |_| run.call(Command::File(FileAction::OpenWith)),
                        }
                    },
                }
                FactList { facts }
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
