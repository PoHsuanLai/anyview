//! A find in a table: the cells of a sheet that hold the phrase, found on a worker window by
//! window (a sheet can have a million rows), the words around each for the palette's list, and
//! where the view puts the cursor to show one.

use super::doc::TableDoc;
use super::view::title;
use crate::families::places::CellHit;
use crate::families::{FoundHits, HitLine, Snippet, SortedOrder, clip};
use crate::{HitIndex, RowNo, SheetNo, Stage, StageIn, TableIn, TypedText};
use anyview_core::work::{Stop, StopState};
use anyview_core::{FindOptions, LineIndex};
use anyview_text::{MAX_HITS, Needle, RowIndex};

/// How many rows one read takes.
const BATCH: u32 = 2048;

/// How many hits keep the words around them, for the palette's list.
const SNIPPETS: usize = 200;

/// The words around a hit in `cell`, which is in the column called `column` of file row `row`:
/// `Row 12 · city: Lima`, the match marked in it.
fn snippet(row: u32, column: &str, cell: &str, hit: &CellHit) -> Snippet {
    let clipped = clip(cell, hit.from as usize, hit.to as usize); // a u32 fits a usize
    let lead = format!("Row {} \u{b7} {column}: ", row.saturating_add(1));
    Snippet {
        matched: lead.len() + clipped.matched.start..lead.len() + clipped.matched.end,
        text: format!("{lead}{}", clipped.text),
        column: clipped.column,
    }
}

/// Every cell of `sheet` of `doc` in which `query` occurs as `options` say, in file order, at most
/// [`MAX_HITS`] of them: blocking, reads the sheet once, a window at a time. None for an empty
/// query or a sheet the file lacks; the hits found so far when `stop` is raised.
pub(crate) fn search_sheet(
    doc: &TableDoc,
    sheet: SheetNo,
    query: &TypedText,
    options: FindOptions,
    stop: &Stop,
) -> FoundHits {
    let (Some(one), Some(needle)) = (
        doc.sheets.get(sheet.0 as usize), // a u32 fits a usize
        Needle::new(query.as_str()).map(|needle| needle.with_options(options)),
    ) else {
        return FoundHits::default();
    };
    let total = one.table.row_count().0;
    let mut hits: Vec<CellHit> = Vec::new();
    let mut snippets: Vec<Snippet> = Vec::new();
    let mut first = 0_u32;
    while first < total && hits.len() < MAX_HITS && stop.stopped() == StopState::Running {
        let end = first.saturating_add(BATCH).min(total);
        let rows = one
            .table
            .rows(RowIndex(first)..RowIndex(end))
            .unwrap_or_default();
        if rows.is_empty() {
            break; // the file is shorter than its index said
        }
        for (row, cells) in (first..).zip(&rows) {
            for (column, cell) in cells.iter().enumerate() {
                for found in needle.hits_in(LineIndex(0), cell) {
                    if hits.len() >= MAX_HITS {
                        break;
                    }
                    let hit = CellHit {
                        row,
                        column: u32::try_from(column).unwrap_or(u32::MAX),
                        from: found.from.0,
                        to: found.to.0,
                    };
                    if snippets.len() < SNIPPETS {
                        snippets.push(snippet(row, &title(one, column), cell, &hit));
                    }
                    hits.push(hit);
                }
            }
        }
        first = end;
    }
    FoundHits::cells(sheet, hits).with_snippets(snippets)
}

/// The rows of the palette's list: each hit by the words around it, and the sheet it is in when
/// the file has several.
pub(super) fn lines(doc: &TableDoc, stage: &Stage, found: &FoundHits, upto: u32) -> Vec<HitLine> {
    let place = match stage {
        Stage::Table(table) if doc.sheets.len() > 1 => doc
            .sheets
            .get(table.sheet().0 as usize) // a u32 fits a usize
            .map(|sheet| sheet.name.clone())
            .unwrap_or_default(),
        Stage::Table(_)
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Tree(_) => String::new(),
    };
    (0..upto.min(found.count().0))
        .map(|index| {
            let snippet = found.snippet(HitIndex(index)).cloned().unwrap_or_default();
            HitLine {
                context: snippet.text,
                matched: snippet.matched,
                place: place.clone(),
            }
        })
        .collect()
}

/// The place on screen of file row `row`: where the sort put it, or its own number when the rows
/// are in the file's order (or the order has not arrived).
fn shown_at(row: u32, sorted: Option<&SortedOrder>) -> u32 {
    sorted
        .and_then(|sorted| sorted.order.iter().position(|at| *at == row))
        .and_then(|at| u32::try_from(at).ok())
        .unwrap_or(row)
}

/// The inputs that show hit `hit`: the cursor goes to its row.
pub(super) fn show(
    stage: &Stage,
    found: &FoundHits,
    sorted: Option<&SortedOrder>,
    hit: HitIndex,
) -> Vec<StageIn> {
    let (Stage::Table(table), Some(cell)) = (stage, found.cell(hit)) else {
        return Vec::new();
    };
    // An order that is for another sheet or another column is not the one the rows are in.
    let sorted =
        sorted.filter(|order| order.sheet == table.sheet() && Some(order.sort) == table.sort());
    vec![StageIn::Table(TableIn::Select(RowNo(shown_at(
        cell.row, sorted,
    ))))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColumnSort, SortDirection};
    use anyview_text::{Coverage, HeaderMode, Table};
    use std::sync::Arc;

    fn doc(rows: &[&[&str]]) -> TableDoc {
        let rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
            .collect();
        let table = Table::from_rows(rows, HeaderMode::Present, Coverage::Whole);
        TableDoc::new(
            vec![Arc::new(super::super::doc::SheetDoc {
                name: String::new(),
                widths: Vec::new(),
                kinds: Vec::new(),
                table,
            })],
            anyview_core::Facts::empty(),
            None,
        )
    }

    #[test]
    fn a_search_finds_every_cell_with_the_phrase_in_file_order_and_says_where() {
        let doc = doc(&[
            &["id", "city", "note"],
            &["1", "Lima", "far away"],
            &["2", "Oslo", "lima beans, Lima"],
            &["3", "Rome", "nothing"],
        ]);
        let found = search_sheet(
            &doc,
            SheetNo(0),
            &TypedText::new("lima"),
            FindOptions::default(),
            &Stop::new(),
        );
        assert_eq!(found.count().0, 3);
        let places: Vec<(u32, u32)> = (0..3)
            .map(|at| {
                let hit = found.cell(HitIndex(at)).unwrap();
                (hit.row, hit.column)
            })
            .collect();
        assert_eq!(places, [(0, 1), (1, 2), (1, 2)]);
        // the words around the first read like the row they are in, the match marked in them
        let first = found.snippet(HitIndex(0)).unwrap();
        assert_eq!(first.text, "Row 1 \u{b7} city: Lima");
        assert_eq!(&first.text[first.matched.clone()], "Lima");
    }

    #[test]
    fn a_search_with_nothing_to_find_finds_nothing() {
        let doc = doc(&[&["a"], &["b"]]);
        for (name, sheet, query) in [
            ("an empty query", SheetNo(0), ""),
            ("a sheet the file lacks", SheetNo(3), "a"),
            ("a phrase that is not there", SheetNo(0), "zzz"),
        ] {
            let found = search_sheet(
                &doc,
                sheet,
                &TypedText::new(query),
                FindOptions::default(),
                &Stop::new(),
            );
            assert_eq!(found.count().0, 0, "{name}");
        }
    }

    #[test]
    fn a_hit_is_shown_where_the_sort_put_its_row() {
        let found = FoundHits::cells(
            SheetNo(0),
            vec![CellHit {
                row: 2,
                column: 0,
                from: 0,
                to: 1,
            }],
        );
        let by = ColumnSort {
            column: 0,
            direction: SortDirection::Ascending,
        };
        let order = SortedOrder {
            sheet: SheetNo(0),
            sort: by,
            order: vec![3, 2, 1, 0],
        };
        let unsorted = Stage::Table(crate::TableStage::default());
        let sorted = Stage::Table(crate::TableStage::Browsing {
            sheet: SheetNo(0),
            sort: Some(by),
            find: None,
            scale: crate::ContentScale::ACTUAL,
            cells: crate::Cells::default(),
        });
        // name, stage, order, row the cursor goes to
        let cases: Vec<(&str, &Stage, Option<&SortedOrder>, u32)> = vec![
            ("rows in the file's order", &unsorted, None, 2),
            (
                "an order that is not the sort's is not used",
                &unsorted,
                Some(&order),
                2,
            ),
            ("the order the sort gave", &sorted, Some(&order), 1),
            ("a sort whose order has not arrived", &sorted, None, 2),
        ];
        for (name, stage, order, row) in cases {
            assert_eq!(
                show(stage, &found, order, HitIndex(0)),
                vec![StageIn::Table(TableIn::Select(RowNo(row)))],
                "{name}"
            );
        }
        assert!(show(&unsorted, &found, None, HitIndex(5)).is_empty());
    }
}
