//! The palette's transitions: quire's machine, and the scope on top of it. The machine knows rows,
//! a query and a highlight; the three things that are anyview's are handled here before it
//! is asked: opening as a find, the Find row, and "Show All", which keep the palette open and
//! change what it lists, and typing in a find, which lists its first hits again.

use super::model::{
    HitList, Palette, PaletteIn, PaletteIndex, PaletteOut, PaletteParams, PaletteScope,
};
use crate::command::{Command, StageCommand};
use ds_core::machine::Machine;
use ds_core::palette::model::PaletteIn as Base;
use ds_core::time::stamp::Stamp;

/// The palette, its scope and what it wants done, after one input.
pub(crate) type ScopedStep = (Palette, PaletteScope, Vec<PaletteOut>);

/// `palette` and its `scope` after `input` over the rows in `params`. The scope is `Commands`
/// whenever the palette is closed.
pub(crate) fn step_with_scope(
    palette: Palette,
    scope: PaletteScope,
    input: PaletteIn,
    at: Stamp,
    params: &PaletteParams,
) -> ScopedStep {
    let (palette, scope, outs) = dispatch(palette, scope, input, at, params);
    let scope = match palette {
        Palette::Open { .. } => scope,
        Palette::Closed => PaletteScope::Commands,
    };
    (palette, scope, outs)
}

fn dispatch(
    palette: Palette,
    scope: PaletteScope,
    input: PaletteIn,
    at: Stamp,
    params: &PaletteParams,
) -> ScopedStep {
    match input {
        PaletteIn::Open => through(palette, scope, Base::Open, at, params),
        PaletteIn::OpenFind(text) => match palette {
            Palette::Closed => {
                let (opened, outs) = palette.step(Base::Open, at, params, &());
                let (typed, _) = opened.step(Base::Typed(text), at, params, &());
                (typed, PaletteScope::Find(HitList::Brief), outs)
            }
            Palette::Open { .. } => to_find(palette, scope),
        },
        PaletteIn::ToFind => to_find(palette, scope),
        // New text is a new list: a find lists its first hits again.
        PaletteIn::Typed(text) => {
            let scope = match scope {
                PaletteScope::Commands => PaletteScope::Commands,
                PaletteScope::Find(_) => PaletteScope::Find(HitList::Brief),
            };
            through(palette, scope, Base::Typed(text), at, params)
        }
        PaletteIn::Move(movement) => through(palette, scope, Base::Move(movement), at, params),
        PaletteIn::Pick(row) => picked(palette, scope, Base::Pick(row), Some(row), at, params),
        PaletteIn::Enter => {
            let row = match &palette {
                Palette::Open { selection, .. } => Some(*selection),
                Palette::Closed => None,
            };
            picked(palette, scope, Base::Enter, row, at, params)
        }
        PaletteIn::Close => through(palette, scope, Base::Close, at, params),
        PaletteIn::Elapsed => through(palette, scope, Base::Elapsed, at, params),
    }
}

/// The input as quire's machine takes it, the scope as it is.
fn through(
    palette: Palette,
    scope: PaletteScope,
    input: Base,
    at: Stamp,
    params: &PaletteParams,
) -> ScopedStep {
    let (palette, outs) = palette.step(input, at, params, &());
    (palette, scope, outs)
}

/// What is typed becomes a find, listing its first hits from the top; a find stays as it is.
fn to_find(palette: Palette, scope: PaletteScope) -> ScopedStep {
    let query = match &palette {
        Palette::Open { query, .. } => query.clone(),
        Palette::Closed => return (palette, scope, vec![]),
    };
    match scope {
        PaletteScope::Commands => (
            Palette::open(query, PaletteIndex(0)),
            PaletteScope::Find(HitList::Brief),
            vec![],
        ),
        PaletteScope::Find(_) => (palette, scope, vec![]),
    }
}

/// `row` chosen with `input`. Two rows keep the palette open: "Show All", which lists every hit,
/// and "Find", which makes the text a find. Any other is the machine's: it runs and closes, and
/// a row that is not there is nothing.
fn picked(
    palette: Palette,
    scope: PaletteScope,
    input: Base,
    row: Option<PaletteIndex>,
    at: Stamp,
    params: &PaletteParams,
) -> ScopedStep {
    let command = row.and_then(|row| params.rows.get(row.0));
    let open = matches!(palette, Palette::Open { .. });
    if open && command == Some(&Command::ShowAllHits) {
        return (palette, PaletteScope::Find(HitList::Whole), vec![]);
    }
    if open && command == Some(&Command::Stage(StageCommand::Find)) {
        return to_find(palette, scope);
    }
    through(palette, scope, input, at, params)
}
