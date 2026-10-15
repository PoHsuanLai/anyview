//! What a viewer window is built on: the edge, the pane seat it has (or none), the shelf of its
//! held results, the measured areas, and the root machine with its dispatch. One hook, called
//! first, so every other part of the window starts from the same wiring.

use super::super::carry::{Carry, carry_out};
use super::super::pane::PaneSeat;
use super::super::press::use_viewer_keys;
use super::super::shelf::{Dispatch, Shelf, use_area, viewer_params};
use crate::families::Area;
use crate::{Chords, Launch, PaneChrome, Presentation, Viewer, ViewerIn};
use dioxus::prelude::*;
use ds::machine::{use_machine_in, use_machine_state};
use ds::prelude::*;
use ds_blitz::{CloseAnswer, use_close_request, use_gpu};
use ds_core::word::Word;

/// The parts of a window every other part reads.
pub(super) struct Wiring {
    /// What the host gave the window.
    pub edge: crate::Edge,
    /// Whether the window is a pane in its host's.
    pub hosted: bool,
    /// Where a pane sits in its host, if it has a seat.
    pub seat: Option<PaneSeat>,
    /// Whose chords are bound.
    pub chords: Chords,
    /// How much chrome a pane draws.
    pub pane_chrome: PaneChrome,
    /// The window's keymap.
    pub keymap: Keys,
    /// The attributes of the frame the window draws in, from the theme.
    pub frame: String,
    /// The window's held results.
    pub shelf: Shelf,
    /// The stage's area.
    pub area: Memo<Option<Area>>,
    /// What measures the stage's area.
    pub measured: ds::host::measure::RectProbe,
    /// The whole window's area.
    pub whole: Memo<Option<Area>>,
    /// What measures the whole window.
    pub whole_probe: ds::host::measure::RectProbe,
    /// What carrying out an output needs.
    pub carry: Carry,
    /// Sends inputs to the root machine.
    pub dispatch: Dispatch,
}

/// Build the wiring of the window for `launch`: the root machine is built here, and the close
/// request is wired to it, so a window with changes that are not saved asks first.
pub(super) fn use_wiring(launch: &Launch) -> Wiring {
    let edge = use_hook(consume_context::<crate::Edge>);
    let hosted = launch.presentation == Presentation::Pane;
    let seat = use_hook(|| {
        if hosted {
            try_consume_context::<PaneSeat>()
        } else {
            None
        }
    });
    let chords = if hosted { Chords::None } else { Chords::Viewer };
    let pane_chrome = seat.map_or_else(PaneChrome::default, |seat| seat.chrome);
    let keymap = use_viewer_keys(chords);
    let gpu = use_gpu();
    let scope = use_scope();
    let scale = try_consume_context::<HostSignals>().map_or(Scale::ONE, |host| (host.scale)());
    let mut shelf = Shelf::empty(edge.platform());
    shelf.pane_chrome = pane_chrome;
    let (area, measured) = use_area(scale);
    let (whole, whole_probe) = use_area(scale);
    let slot = use_hook(|| CopyValue::new(None));
    let carry = Carry {
        shelf,
        edge: edge.clone(),
        gpu: gpu.clone(),
        toasts: ds::prelude::use_toasts(),
        machine: slot,
        seat,
    };
    let handler = carry.clone();
    let presentation = launch.presentation;
    let held = use_machine_state::<Viewer>(move |_| Viewer::launched(presentation));
    let machine = use_machine_in(
        held,
        (),
        move || viewer_params(&held.state.peek(), shelf, area),
        move |out, _| carry_out(out, &handler),
    );
    let dispatch = Dispatch {
        machine,
        shelf,
        area,
    };
    let mut slot = slot;
    slot.set(Some(dispatch));
    // The frame's close button and the compositor's close leave as ⌘W does: a window with changes
    // that are not saved asks Save, Don't Save or Cancel first, and the app closes it itself once
    // the person has answered.
    // A pane has no window of its own to close: the handler would take the host's.
    if !hosted {
        use_close_request(move || {
            if dispatch.machine.state().peek().unsaved() {
                dispatch.send(ViewerIn::CloseRequested);
                CloseAnswer::Keep
            } else {
                CloseAnswer::Close
            }
        });
    }
    Wiring {
        edge,
        hosted,
        seat,
        chords,
        pane_chrome,
        keymap,
        frame: format!(
            "data-theme=\"{}\" data-accent=\"{}\" data-motion=\"{}\" data-material=\"window\"",
            scope.scheme.slug(),
            scope.resolved.accent.slug(),
            scope.resolved.motion.slug(),
        ),
        shelf,
        area,
        measured,
        whole,
        whole_probe,
        carry,
        dispatch,
    }
}
