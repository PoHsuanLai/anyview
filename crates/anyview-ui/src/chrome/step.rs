//! The chrome's transitions: an outer match on the state, an inner one on the input.

use super::model::{Chrome, ChromeIn, ChromeOut, ChromeParams, Zone};
use super::pins::{PinReason, PinReasons};
use crate::time::after;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::Shown;

type Step = (Chrome, Vec<ChromeOut>);

impl Machine for Chrome {
    type In = ChromeIn;
    type Out = ChromeOut;
    type Params = ChromeParams;
    type Ctx = ();

    fn step(self, input: ChromeIn, at: Stamp, params: &ChromeParams, _cx: &()) -> Step {
        match self {
            Chrome::Hidden => hidden(self, input, at, params),
            Chrome::Revealing { since: _, until } => revealing(self, input, at, until, params),
            Chrome::Shown {
                idle_from: _,
                hide_at,
            } => shown(self, input, at, hide_at, params),
            Chrome::Pinned { by } => pinned(self, input, at, by, params),
            Chrome::Hiding { since: _, until } => hiding(self, input, at, until, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Chrome::Hidden | Chrome::Pinned { by: _ } => None,
            Chrome::Revealing { since: _, until } | Chrome::Hiding { since: _, until } => {
                Some(*until)
            }
            Chrome::Shown {
                idle_from: _,
                hide_at,
            } => Some(*hide_at),
        }
    }
}

fn fade_in(params: &ChromeParams) -> ChromeOut {
    ChromeOut::Fade {
        to: Shown::Visible,
        over: params.reveal,
    }
}

fn revealing_from(at: Stamp, params: &ChromeParams) -> Step {
    let state = Chrome::Revealing {
        since: at,
        until: after(at, params.reveal),
    };
    (state, vec![fade_in(params)])
}

fn shown_from(idle_from: Stamp, params: &ChromeParams) -> Chrome {
    Chrome::Shown {
        idle_from,
        hide_at: after(idle_from, params.hide_after),
    }
}

fn held(reason: PinReason) -> Chrome {
    Chrome::Pinned {
        by: PinReasons::of(reason),
    }
}

/// `by` after `reason` ended: still held if another reason remains, otherwise shown and idle.
fn released(by: PinReasons, reason: PinReason, at: Stamp, params: &ChromeParams) -> Step {
    match by.without(reason) {
        Some(rest) => (Chrome::Pinned { by: rest }, vec![]),
        None => (shown_from(at, params), vec![]),
    }
}

fn hidden(this: Chrome, input: ChromeIn, at: Stamp, params: &ChromeParams) -> Step {
    match input {
        ChromeIn::PointerMoved(Zone::Content) => revealing_from(at, params),
        ChromeIn::PointerMoved(Zone::Capsule) => {
            (held(PinReason::PointerOverCapsule), vec![fade_in(params)])
        }
        ChromeIn::Pin(reason) => (held(reason), vec![fade_in(params)]),
        ChromeIn::PointerLeft | ChromeIn::Unpin(_) | ChromeIn::Elapsed => (this, vec![]),
    }
}

fn revealing(
    this: Chrome,
    input: ChromeIn,
    at: Stamp,
    until: Stamp,
    params: &ChromeParams,
) -> Step {
    match input {
        ChromeIn::PointerMoved(Zone::Capsule) => (held(PinReason::PointerOverCapsule), vec![]),
        ChromeIn::Pin(reason) => (held(reason), vec![]),
        ChromeIn::Elapsed if at >= until => (shown_from(at, params), vec![]),
        ChromeIn::PointerMoved(Zone::Content)
        | ChromeIn::PointerLeft
        | ChromeIn::Unpin(_)
        | ChromeIn::Elapsed => (this, vec![]),
    }
}

fn shown(this: Chrome, input: ChromeIn, at: Stamp, hide_at: Stamp, params: &ChromeParams) -> Step {
    match input {
        ChromeIn::PointerMoved(Zone::Content) => (shown_from(at, params), vec![]),
        ChromeIn::PointerMoved(Zone::Capsule) => (held(PinReason::PointerOverCapsule), vec![]),
        ChromeIn::Pin(reason) => (held(reason), vec![]),
        ChromeIn::Elapsed if at >= hide_at => {
            let state = Chrome::Hiding {
                since: at,
                until: after(at, params.hide),
            };
            let out = ChromeOut::Fade {
                to: Shown::Hidden,
                over: params.hide,
            };
            (state, vec![out])
        }
        ChromeIn::PointerLeft | ChromeIn::Unpin(_) | ChromeIn::Elapsed => (this, vec![]),
    }
}

fn pinned(this: Chrome, input: ChromeIn, at: Stamp, by: PinReasons, params: &ChromeParams) -> Step {
    match input {
        ChromeIn::PointerMoved(Zone::Content) | ChromeIn::PointerLeft => {
            released(by, PinReason::PointerOverCapsule, at, params)
        }
        ChromeIn::PointerMoved(Zone::Capsule) => {
            let by = by.with(PinReason::PointerOverCapsule);
            (Chrome::Pinned { by }, vec![])
        }
        ChromeIn::Pin(reason) => (
            Chrome::Pinned {
                by: by.with(reason),
            },
            vec![],
        ),
        ChromeIn::Unpin(reason) => released(by, reason, at, params),
        ChromeIn::Elapsed => (this, vec![]),
    }
}

fn hiding(this: Chrome, input: ChromeIn, at: Stamp, until: Stamp, params: &ChromeParams) -> Step {
    match input {
        ChromeIn::PointerMoved(Zone::Content) => revealing_from(at, params),
        ChromeIn::PointerMoved(Zone::Capsule) => {
            (held(PinReason::PointerOverCapsule), vec![fade_in(params)])
        }
        ChromeIn::Pin(reason) => (held(reason), vec![fade_in(params)]),
        ChromeIn::Elapsed if at >= until => (Chrome::Hidden, vec![]),
        ChromeIn::PointerLeft | ChromeIn::Unpin(_) | ChromeIn::Elapsed => (this, vec![]),
    }
}
