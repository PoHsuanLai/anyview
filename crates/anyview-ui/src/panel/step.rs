//! The panel's transitions.

use super::model::{Panel, PanelIn, PanelOut, PanelParams, PanelTab};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Panel, Vec<PanelOut>);

impl Machine for Panel {
    type In = PanelIn;
    type Out = PanelOut;
    type Params = PanelParams;

    fn step(self, input: PanelIn, _at: Stamp, params: &PanelParams) -> Step {
        match self {
            Panel::Hidden => hidden(input, params),
            Panel::Shown { tab } => shown(self, tab, input, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Panel::Hidden | Panel::Shown { tab: _ } => None,
        }
    }
}

fn show(tab: PanelTab) -> Step {
    (Panel::Shown { tab }, vec![PanelOut::Show(tab)])
}

fn hide() -> Step {
    (Panel::Hidden, vec![PanelOut::Hide])
}

fn hidden(input: PanelIn, params: &PanelParams) -> Step {
    match input {
        PanelIn::Toggle => params.tabs.first().map_or((Panel::Hidden, vec![]), show),
        PanelIn::Choose(tab) if params.tabs.contains(tab) => show(tab),
        PanelIn::Choose(_) | PanelIn::Close | PanelIn::TabsChanged | PanelIn::Elapsed => {
            (Panel::Hidden, vec![])
        }
    }
}

fn shown(this: Panel, tab: PanelTab, input: PanelIn, params: &PanelParams) -> Step {
    match input {
        PanelIn::Toggle | PanelIn::Close => hide(),
        PanelIn::Choose(chosen) if chosen == tab => (this, vec![]),
        PanelIn::Choose(chosen) if params.tabs.contains(chosen) => show(chosen),
        PanelIn::TabsChanged if !params.tabs.contains(tab) => {
            params.tabs.first().map_or_else(hide, show)
        }
        PanelIn::Choose(_) | PanelIn::TabsChanged | PanelIn::Elapsed => (this, vec![]),
    }
}
