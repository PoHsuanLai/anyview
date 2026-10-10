//! The palette's transitions.

use super::model::{
    HitList, Palette, PaletteIn, PaletteIndex, PaletteMove, PaletteOut, PaletteParams, PaletteScope,
};
use crate::command::{Command, StageCommand};
use crate::typed::TypedText;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Palette, Vec<PaletteOut>);

impl Machine for Palette {
    type In = PaletteIn;
    type Out = PaletteOut;
    type Params = PaletteParams;
    type Ctx = ();

    fn step(self, input: PaletteIn, _at: Stamp, params: &PaletteParams, _cx: &()) -> Step {
        match self {
            Palette::Closed => closed(input),
            Palette::Open {
                query,
                selection,
                scope,
            } => open(query, selection, scope, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Palette::Closed
            | Palette::Open {
                query: _,
                selection: _,
                scope: _,
            } => None,
        }
    }
}

fn closed(input: PaletteIn) -> Step {
    match input {
        PaletteIn::Open => {
            let state = Palette::Open {
                query: TypedText::EMPTY,
                selection: PaletteIndex(0),
                scope: PaletteScope::Commands,
            };
            (state, vec![PaletteOut::Opened])
        }
        PaletteIn::OpenFind(query) => {
            let state = Palette::Open {
                query,
                selection: PaletteIndex(0),
                scope: PaletteScope::Find(HitList::Brief),
            };
            (state, vec![PaletteOut::Opened])
        }
        PaletteIn::ToFind
        | PaletteIn::Typed(_)
        | PaletteIn::Move(_)
        | PaletteIn::Pick(_)
        | PaletteIn::Enter
        | PaletteIn::Close
        | PaletteIn::Elapsed => (Palette::Closed, vec![]),
    }
}

fn open(
    query: TypedText,
    selection: PaletteIndex,
    scope: PaletteScope,
    input: PaletteIn,
    params: &PaletteParams,
) -> Step {
    let this = Palette::Open {
        query: query.clone(),
        selection,
        scope,
    };
    match input {
        // New text is a new list: a find lists its first hits again.
        PaletteIn::Typed(text) => {
            let scope = match scope {
                PaletteScope::Commands => PaletteScope::Commands,
                PaletteScope::Find(_) => PaletteScope::Find(HitList::Brief),
            };
            let state = Palette::Open {
                query: text,
                selection: PaletteIndex(0),
                scope,
            };
            (state, vec![])
        }
        PaletteIn::Move(movement) => {
            let selection = moved(selection, movement, params.rows.len());
            (
                Palette::Open {
                    query,
                    selection,
                    scope,
                },
                vec![],
            )
        }
        PaletteIn::ToFind | PaletteIn::OpenFind(_) => match scope {
            PaletteScope::Commands => (
                Palette::Open {
                    query,
                    selection: PaletteIndex(0),
                    scope: PaletteScope::Find(HitList::Brief),
                },
                vec![],
            ),
            PaletteScope::Find(_) => (this, vec![]),
        },
        PaletteIn::Enter => run(this, selection, params),
        PaletteIn::Pick(row) => run(this, row, params),
        PaletteIn::Close => (Palette::Closed, vec![PaletteOut::Closed]),
        PaletteIn::Open | PaletteIn::Elapsed => (this, vec![]),
    }
}

/// `row`'s command, with the palette closed after it; nothing when there is no such row. Two rows
/// keep the palette open: "Show All", which lists every hit, and "Find", which makes the text a
/// find.
fn run(this: Palette, row: PaletteIndex, params: &PaletteParams) -> Step {
    let Palette::Open {
        query,
        selection,
        scope,
    } = this.clone()
    else {
        return (this, vec![]);
    };
    match params.rows.get(row.0) {
        Some(Command::ShowAllHits) => (
            Palette::Open {
                query,
                selection,
                scope: PaletteScope::Find(HitList::Whole),
            },
            vec![],
        ),
        Some(Command::Stage(StageCommand::Find)) => match scope {
            PaletteScope::Commands => (
                Palette::Open {
                    query,
                    selection: PaletteIndex(0),
                    scope: PaletteScope::Find(HitList::Brief),
                },
                vec![],
            ),
            PaletteScope::Find(_) => (this, vec![]),
        },
        Some(command) => (
            Palette::Closed,
            vec![PaletteOut::Run(*command), PaletteOut::Closed],
        ),
        None => (this, vec![]),
    }
}

/// The highlight after `movement` over `rows` rows, clamped to the first and last.
fn moved(selection: PaletteIndex, movement: PaletteMove, rows: usize) -> PaletteIndex {
    let last = rows.saturating_sub(1);
    let target = match movement {
        PaletteMove::Up => selection.0.saturating_sub(1),
        PaletteMove::Down => selection.0.saturating_add(1),
        PaletteMove::First => 0,
        PaletteMove::Last => last,
    };
    PaletteIndex(target.min(last))
}
