//! The palette's transitions.

use super::model::{Palette, PaletteIn, PaletteMove, PaletteOut, PaletteParams, RowIndex};
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Palette, Vec<PaletteOut>);

impl Machine for Palette {
    type In = PaletteIn;
    type Out = PaletteOut;
    type Params = PaletteParams;

    fn step(self, input: PaletteIn, _at: Stamp, params: &PaletteParams) -> Step {
        match self {
            Palette::Closed => closed(input),
            Palette::Open { query, selection } => open(query, selection, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Palette::Closed
            | Palette::Open {
                query: _,
                selection: _,
            } => None,
        }
    }
}

fn closed(input: PaletteIn) -> Step {
    match input {
        PaletteIn::Open => {
            let state = Palette::Open {
                query: TypedText::EMPTY,
                selection: RowIndex(0),
            };
            (state, vec![PaletteOut::Opened])
        }
        PaletteIn::Typed(_)
        | PaletteIn::Move(_)
        | PaletteIn::Pick(_)
        | PaletteIn::Enter
        | PaletteIn::Close
        | PaletteIn::Elapsed => (Palette::Closed, vec![]),
    }
}

fn open(query: TypedText, selection: RowIndex, input: PaletteIn, params: &PaletteParams) -> Step {
    let this = Palette::Open {
        query: query.clone(),
        selection,
    };
    match input {
        PaletteIn::Typed(text) => {
            let state = Palette::Open {
                query: text,
                selection: RowIndex(0),
            };
            (state, vec![])
        }
        PaletteIn::Move(movement) => {
            let selection = moved(selection, movement, params.rows.len());
            (Palette::Open { query, selection }, vec![])
        }
        PaletteIn::Enter => run(this, selection, params),
        PaletteIn::Pick(row) => run(this, row, params),
        PaletteIn::Close => (Palette::Closed, vec![PaletteOut::Closed]),
        PaletteIn::Open | PaletteIn::Elapsed => (this, vec![]),
    }
}

/// `row`'s command, with the palette closed after it; nothing when there is no such row.
fn run(this: Palette, row: RowIndex, params: &PaletteParams) -> Step {
    match params.rows.get(row.0) {
        Some(command) => (
            Palette::Closed,
            vec![PaletteOut::Run(*command), PaletteOut::Closed],
        ),
        None => (this, vec![]),
    }
}

/// The highlight after `movement` over `rows` rows, clamped to the first and last.
fn moved(selection: RowIndex, movement: PaletteMove, rows: usize) -> RowIndex {
    let last = rows.saturating_sub(1);
    let target = match movement {
        PaletteMove::Up => selection.0.saturating_sub(1),
        PaletteMove::Down => selection.0.saturating_add(1),
        PaletteMove::First => 0,
        PaletteMove::Last => last,
    };
    RowIndex(target.min(last))
}
