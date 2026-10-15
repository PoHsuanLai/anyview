//! Delimited tables and spreadsheets: the table stage's view. A delimited file is indexed on a
//! worker (`doc`) and a workbook read there; the rows on screen are the ones a `VirtualTable` mounts
//! (`view`), parsed from the file a window at a time.

mod doc;
mod edits;
mod find;
mod kind;
mod panel;
mod pick;
mod sort;
mod view;

pub use doc::{SheetDoc, TableDoc};
pub(crate) use find::search_sheet;
pub use sort::SortedOrder;
pub(crate) use sort::sort_sheet;

use crate::families::FoundHits;
use crate::families::view::{Area, EditDone, Held, HitLine, StageCx, StageView};
use crate::io::{Job, OpenError, OpenPort};
use crate::{
    Command, EditOffer, Editable, HitIndex, PanelTab, PanelTabs, SheetNo, SheetTotal, Stage,
    StageCommand, StageFamily, StageIn, StageOut, StageParams, TableIn, TableParams, Ticket,
    TypedText,
};
use anyview_core::work::Stop;
use anyview_core::{Facts, FindOptions, Sniffed, Source};
use anyview_text::EditStamp;
use dioxus::prelude::*;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::{RankedSlot, essentials};
use ds::prelude::Icon;
use ds_core::selection::RowSelection;
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
        stop: &Stop,
    ) -> Result<TableDoc, OpenError> {
        doc::open(src, sniffed, stop)
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
        let scale = match stage {
            Stage::Table(stage) => stage.scale(),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Tree(_) => crate::ContentScale::ACTUAL,
        };
        StageParams::default().with_table(TableParams {
            columns: doc
                .sheets
                .get(at)
                .map_or(0, |sheet| sheet.table.columns().0),
            sheets: SheetTotal(u32::try_from(doc.sheets.len()).unwrap_or(u32::MAX)),
            rows: doc.rows_now(at),
            page: view::page_of(area, scale),
            editable: if doc.is_editable() {
                Editable::Yes
            } else {
                Editable::No
            },
        })
    }

    fn arrived(doc: &TableDoc, stage: &Stage, _left_at: &anyview_core::Resume) -> Vec<StageIn> {
        // What the stage holds of the edits is what the document that has landed holds.
        let held = doc.edits();
        let log = held.as_ref().map_or(crate::EditLog::NONE, edits::log_of);
        let mut told = vec![StageIn::Table(TableIn::Session(
            crate::EditSessionIn::Logged(log),
        ))];
        // A find that was up is asked again of the document that has landed.
        if let Stage::Table(table) = stage
            && let Some(find) = table.find()
        {
            told.push(StageIn::Table(TableIn::Find(find.query.clone())));
        }
        told
    }

    fn copy_text(
        doc: &Arc<TableDoc>,
        stage: &Stage,
        rows: &RowSelection,
        sorted: Option<&SortedOrder>,
    ) -> Option<String> {
        let Stage::Table(stage) = stage else {
            return None;
        };
        let sheet = doc.sheets.get(stage.sheet().0 as usize)?; // a u32 fits a usize
        // An order that is for another sheet or another column is not the one the rows are in.
        let order = sorted
            .filter(|order| order.sheet == stage.sheet() && Some(order.sort) == stage.sort())
            .map(|order| order.order.as_slice());
        // The cells as they are with the edits made to them.
        let held = doc.edits();
        match (stage.sheet(), held.as_ref()) {
            (SheetNo(0), Some(live)) => {
                let columns = sheet.table.columns().0 as usize; // a u32 fits a usize
                Some(pick::copy_read(columns, rows, |place, column| {
                    edits::text_at((live, &sheet.table), order, place, column)
                }))
            }
            (_, Some(_) | None) => Some(pick::copy_text(sheet, rows, order)),
        }
    }

    fn edit(
        doc: &Arc<TableDoc>,
        ticket: Ticket,
        stage: &Stage,
        out: &StageOut,
        order: Option<&SortedOrder>,
    ) -> EditDone {
        edits::carry(doc, ticket, stage, out, order)
    }

    fn written(doc: &TableDoc, stamp: EditStamp) -> Vec<StageIn> {
        let mut held = doc.edits();
        match held.as_mut() {
            Some(live) => {
                live.mark_saved(stamp);
                vec![StageIn::Table(TableIn::Session(
                    crate::EditSessionIn::Logged(edits::log_of(live)),
                ))]
            }
            None => Vec::new(),
        }
    }

    fn edit_offer(doc: &TableDoc) -> EditOffer {
        if doc.is_editable() {
            EditOffer::Plain
        } else {
            EditOffer::Withheld
        }
    }

    fn stage(doc: &Arc<TableDoc>, cx: &StageCx) -> Element {
        rsx! { view::TableContent { doc: Held(Arc::clone(doc)), cx: cx.clone() } }
    }

    fn slots(doc: &TableDoc, cx: &StageCx) -> Vec<RankedSlot<Command>> {
        let found = crate::families::found::standing(&cx.stage);
        if doc.sheets.len() < 2 {
            return found;
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
        let mut slots = essentials(vec![
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
        ]);
        slots.extend(found);
        slots
    }

    fn search(
        doc: &Arc<TableDoc>,
        stage: &Stage,
        ticket: Ticket,
        query: &TypedText,
        options: FindOptions,
    ) -> Option<Job> {
        let Stage::Table(table) = stage else {
            return None;
        };
        Some(Job::SearchCells {
            ticket,
            doc: Arc::clone(doc),
            sheet: table.sheet(),
            query: query.clone(),
            options,
        })
    }

    fn show_hit(
        _doc: &Arc<TableDoc>,
        stage: &Stage,
        hits: &FoundHits,
        sorted: Option<&SortedOrder>,
        hit: HitIndex,
    ) -> Vec<StageIn> {
        find::show(stage, hits, sorted, hit)
    }

    fn hit_lines(doc: &Arc<TableDoc>, cx: &StageCx, upto: u32) -> Vec<HitLine> {
        match cx.hits.as_ref() {
            Some(found) => find::lines(doc, &cx.stage, &found.0, upto),
            None => Vec::new(),
        }
    }

    fn panel(doc: &Arc<TableDoc>, tab: PanelTab, cx: &StageCx) -> Option<Element> {
        panel::body(doc, tab, cx)
    }
}
