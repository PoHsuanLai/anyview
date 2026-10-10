//! Delimited tables and spreadsheets: the table stage's view. A file is read whole on a worker
//! (`doc`), up to a cap; the rows on screen are the ones a `VirtualTable` mounts (`view`).

mod doc;
mod panel;
mod view;

pub use doc::{SheetDoc, TableDoc};

use crate::families::view::{Area, Held, StageCx, StageView};
use crate::io::{OpenError, OpenPort};
use crate::{
    Command, PanelTab, PanelTabs, SheetNo, SheetTotal, Stage, StageCommand, StageFamily,
    StageParams, TableParams, Ticket,
};
use anyview_core::{Facts, Sniffed, Source};
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::{RankedSlot, essentials};
use ds::prelude::Icon;
use std::sync::Arc;

/// CSV, TSV, XLSX, ODS and XLS files, shown as rows under a header.
#[derive(Debug, Clone, Copy)]
pub struct TableStageView;

impl StageView for TableStageView {
    const FAMILY: StageFamily = StageFamily::Table;
    type Doc = TableDoc;

    fn open(
        _ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        _link: &OpenPort,
    ) -> Result<TableDoc, OpenError> {
        doc::open(src, sniffed)
    }

    fn facts(doc: &TableDoc) -> Facts {
        doc.facts.clone()
    }

    fn tabs(doc: &TableDoc) -> PanelTabs {
        if doc.sheets.len() > 1 {
            PanelTabs::of(&[PanelTab::Sheets, PanelTab::Info])
        } else {
            PanelTabs::of(&[PanelTab::Info])
        }
    }

    fn params(doc: &TableDoc, stage: &Stage, area: Option<Area>) -> StageParams {
        let at = match stage {
            Stage::Table(stage) => stage.sheet().0 as usize, // a u32 fits a usize
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Tree(_) => 0,
        };
        StageParams {
            table: TableParams {
                columns: doc
                    .sheets
                    .get(at)
                    .map_or(0, |sheet| sheet.table.columns().0),
                sheets: SheetTotal(u32::try_from(doc.sheets.len()).unwrap_or(u32::MAX)),
                rows: doc
                    .sheets
                    .get(at)
                    .map_or(0, |sheet| sheet.table.row_count().0),
                page: view::page_of(area),
            },
            ..StageParams::default()
        }
    }

    fn stage(doc: &Arc<TableDoc>, cx: &StageCx) -> Element {
        rsx! { view::TableContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &TableDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
        if doc.sheets.len() < 2 {
            return Vec::new();
        }
        let SheetNo(at) = match &cx.stage {
            Stage::Table(stage) => stage.sheet(),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Tree(_) => SheetNo(0),
        };
        let name = doc
            .sheets
            .get(at as usize) // a u32 fits a usize
            .map_or("", |sheet| sheet.name.as_str());
        essentials(vec![
            CapsuleSlot::button(
                Command::Stage(StageCommand::PreviousSheet),
                "Previous Sheet",
                Icon::ChevronLeft,
            ),
            CapsuleSlot::Readout(format!("{} of {}: {name}", at + 1, doc.sheets.len())),
            CapsuleSlot::button(
                Command::Stage(StageCommand::NextSheet),
                "Next Sheet",
                Icon::ChevronRight,
            ),
        ])
    }

    fn panel(doc: &Arc<TableDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        panel::body(doc, tab, cx)
    }
}
