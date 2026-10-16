//! A find's results arriving: the hits a worker made, as the stage input that suits the family.

use super::super::carry::Carry;
use crate::families::{FoundHits, Held};
use crate::{
    HitCount, HitIndex, Stage, StageIn, TableIn, TextIn, TextStage, Ticket, TreeIn, TypedText,
    ViewerIn,
};
use anyview_core::{FindOptions, LineIndex};
use dioxus::prelude::*;
use std::sync::Arc;

/// How a stage is told its search answered.
type Answer = fn(TypedText, HitCount, HitIndex) -> StageIn;

fn text_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Text(TextIn::Results {
        query,
        count,
        nearest,
    })
}

fn table_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Table(TableIn::Results {
        query,
        count,
        nearest,
    })
}

fn tree_results(query: TypedText, count: HitCount, nearest: HitIndex) -> StageIn {
    StageIn::Tree(TreeIn::Results {
        query,
        count,
        nearest,
    })
}

/// The find the stage has up, and what the answer to it needs: the text searched for, the line the
/// reader is at (the hit nearest it is the one to show first), and how the stage hears the answer.
/// `None` when no find is up, or when `hits` are of something the stage no longer shows (a table's
/// hits are of one sheet).
fn asking(stage: &Stage, hits: &FoundHits) -> Option<(TypedText, LineIndex, Answer)> {
    match stage {
        Stage::Text(TextStage::Finding { query, place, .. }) => {
            Some((query.clone(), place.line, text_results as Answer))
        }
        Stage::Table(table) => {
            let find = table.find()?;
            // The rows a cursor is on are where the sort put them: only the file's own order
            // says which hit a reader is near.
            let row = table
                .row()
                .filter(|_| table.sort().is_none())
                .map_or(0, |row| row.0);
            hits.sheet()
                .is_none_or(|sheet| sheet == table.sheet())
                .then(|| (find.query.clone(), LineIndex(row), table_results as Answer))
        }
        Stage::Tree(tree) => {
            let find = tree.find()?;
            Some((find.query.clone(), LineIndex(0), tree_results as Answer))
        }
        Stage::Text(TextStage::Reading { .. } | TextStage::Editing { .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Pdf(_)
        | Stage::Media(_) => None,
    }
}

/// A search answered: the hits are kept for the view, and the stage is told how many there are
/// and which is nearest the reader.
pub(super) fn found(c: &Carry, ticket: Ticket, asked: (TypedText, FindOptions), hits: FoundHits) {
    let (query, options) = asked;
    let mut shelf = c.shelf;
    if shelf.loaded.peek().as_ref().map(|(held, _)| *held) != Some(ticket) {
        return;
    }
    let Some(dispatch) = c.dispatch() else {
        return;
    };
    let (stage, current) = {
        let signal = dispatch.machine.state();
        let state = signal.peek();
        (state.stage.clone(), state.find_options)
    };
    let Some((up, at, answer)) = asking(&stage, &hits) else {
        return;
    };
    // An answer for another text, or for options since switched, is no answer to the find now up.
    if up != query || current != options {
        return;
    }
    let (count, nearest) = (hits.count(), hits.nearest(at));
    shelf.hits.set(Some(Held(Arc::new(hits))));
    dispatch.send(ViewerIn::Stage(answer(query, count, nearest)));
}
