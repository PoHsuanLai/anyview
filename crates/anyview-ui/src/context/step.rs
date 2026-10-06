//! The context menu's transitions.

use super::model::{ContextEntry, ContextIn, ContextMenu, ContextOut, ContextParams, ContextPick};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (ContextMenu, Vec<ContextOut>);

impl Machine for ContextMenu {
    type In = ContextIn;
    type Out = ContextOut;
    type Params = ContextParams;
    type Ctx = ();

    fn step(self, input: ContextIn, _at: Stamp, params: &ContextParams, _cx: &()) -> Step {
        match self {
            ContextMenu::Closed => closed(input, params),
            ContextMenu::Open { at } => open(self, at, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            ContextMenu::Closed | ContextMenu::Open { at: _ } => None,
        }
    }
}

fn closed(input: ContextIn, params: &ContextParams) -> Step {
    if params.entries.is_empty() {
        return (ContextMenu::Closed, vec![]);
    }
    match input {
        ContextIn::Open(at) => (ContextMenu::Open { at }, vec![]),
        ContextIn::OpenAtCentre => (ContextMenu::Open { at: params.centre }, vec![]),
        ContextIn::Pick(_) | ContextIn::Close | ContextIn::Elapsed => (ContextMenu::Closed, vec![]),
    }
}

fn open(
    this: ContextMenu,
    at: super::model::Spot,
    input: ContextIn,
    params: &ContextParams,
) -> Step {
    match input {
        ContextIn::Open(spot) => (ContextMenu::Open { at: spot }, vec![]),
        ContextIn::OpenAtCentre => (ContextMenu::Open { at: params.centre }, vec![]),
        ContextIn::Pick(pick) if listed(pick, params) => {
            (ContextMenu::Open { at }, vec![ContextOut::Run(pick)])
        }
        ContextIn::Close => (ContextMenu::Closed, vec![]),
        ContextIn::Pick(_) | ContextIn::Elapsed => (this, vec![]),
    }
}

/// Whether `pick` is a row of the menu.
fn listed(pick: ContextPick, params: &ContextParams) -> bool {
    params.entries.iter().any(|entry| match entry {
        ContextEntry::Item {
            pick: row,
            title: _,
        } => *row == pick,
        ContextEntry::Separator => false,
    })
}
