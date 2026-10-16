//! The effects of the viewer window that are not about drawing: what a pane does for its host,
//! where the keyboard goes back to, the mailbox of what workers made, and the menu closing as the
//! window is resized. Each is a hook, called at one place in the window, always.

use super::super::arrive::arrived;
use super::super::carry::Carry;
use super::super::pane::{PaneSeat, after_events};
use super::super::shelf::{Dispatch, Shelf};
use crate::io::{HostRequest, MediaSupport};
use crate::{Command, ContextIn, ContextMenu, Palette as PaletteState, TypedText, ViewerIn};
use anyview_core::FormatKind;
use dioxus::prelude::*;
use ds::focus::soon::focus_soon;
use ds::prelude::Size;
use futures_util::StreamExt;
use std::rc::Rc;

/// Whether a pane's host has given it the keyboard: a window, which has no seat, always has it.
pub(super) fn host_has_focus(seat: Option<PaneSeat>) -> bool {
    seat.is_none_or(|seat| (seat.focused)())
}

/// What a pane does for its host: takes the keyboard when the host gives it, lists its commands in
/// the host's palette and runs them (and the host's find) through the machine, and withdraws when
/// it goes. `root` is the window's root element, which gets the focus.
pub(super) fn use_pane_host(
    seat: Option<PaneSeat>,
    dispatch: Dispatch,
    root: Signal<Option<Rc<MountedData>>>,
    hosted: bool,
) {
    // The host gives the pane the keyboard: the root takes the focus, as a window's does when it
    // opens. (The pane never takes it unasked.)
    use_effect(move || {
        if hosted
            && host_has_focus(seat)
            && let Some(element) = root.peek().clone()
        {
            focus_soon(element);
        }
    });
    // The host's palette lists what the pane can do, and runs it through the machine's own path.
    use_effect(move || {
        if let Some(seat) = seat {
            seat.link.serve(Callback::new(move |command: Command| {
                dispatch.send(ViewerIn::Run(command));
            }));
            // A find the host's palette feeds goes to the stage as the window's own palette
            // would send it (`find_input`, `dismissal`), without opening a palette.
            seat.link
                .serve_find(Callback::new(move |query: Option<TypedText>| {
                    let input = {
                        let state = dispatch.machine.state();
                        let viewer = state.peek();
                        match &query {
                            Some(query) => viewer.stage.find_input(query),
                            None if viewer.stage.is_finding() => viewer.stage.dismissal(),
                            None => None,
                        }
                    };
                    if let Some(input) = input {
                        dispatch.send(ViewerIn::Stage(input));
                    }
                }));
        }
    });
    use_drop(move || {
        if let Some(seat) = seat {
            seat.link.withdraw();
        }
    });
}

/// What workers made comes back through the mailbox, one input at a time.
pub(super) fn use_mailbox(edge: &crate::Edge, carry: &Carry) {
    let mailbox = edge.clone();
    let arriving = carry.clone();
    use_future(move || {
        let taken = mailbox.take_mailbox();
        let carry = arriving.clone();
        async move {
            let Some(mut inbox) = taken else { return };
            while let Some(done) = inbox.next().await {
                arrived(done, &carry);
            }
        }
    });
}

/// Where the keyboard goes back to: the window's root once a value typed into a cell or a node is
/// taken or let go, and once the palette that held the keys closes (or the text being edited, when
/// there is one).
pub(super) fn use_focus_return(
    root: Signal<Option<Rc<MountedData>>>,
    editor: ds::edit::handle::EditHandle,
    dispatch: Dispatch,
    typing_now: bool,
    palette_open: bool,
) {
    // The keys come back to the window once a value typed into a cell or a node is taken or let go.
    let mut was_typing = use_signal(|| false);
    use_effect(use_reactive!(|typing_now| {
        let before = *was_typing.peek();
        was_typing.set(typing_now);
        if before
            && !typing_now
            && let Some(element) = root.peek().clone()
        {
            focus_soon(element);
        }
    }));
    // The keys belong to the window again once the palette that held them is closed.
    use_effect(use_reactive!(|palette_open| {
        if palette_open {
            return;
        }
        if dispatch.machine.state().peek().stage.edited().is_some() {
            // The text being edited has the keyboard back.
            editor.focus();
        } else if let Some(element) = root.peek().clone() {
            focus_soon(element);
        }
    }));
}

/// An open menu closes when the window changes size, as a Mac's or GNOME's does: it was placed for
/// the window it opened in. `size_now` is the window's size as of this render.
pub(super) fn use_close_on_resize(size_now: Option<Size>, dispatch: Dispatch) {
    let sized = use_hook(|| CopyValue::new(size_now));
    use_effect(use_reactive!(|size_now| {
        let mut sized = sized;
        let before = std::mem::replace(&mut *sized.write(), size_now);
        let open = matches!(
            dispatch.machine.state().peek().context,
            ContextMenu::Open { .. }
        );
        if open && before.is_some() && before != size_now {
            dispatch.send(ViewerIn::Context(ContextIn::Close));
        }
    }));
}

/// The focus left a pane's root: tells the host, unless the focus went to another part of the pane
/// (a `focusin` the root hears right after) or to a menu or the palette of the pane's own. A layer
/// that closed took the focus with it, and then the root takes it back instead. `entered` counts
/// the focus coming in, `dropped_layer` is set when such a layer closed.
pub(super) fn focus_left(
    seat: Option<PaneSeat>,
    entered: CopyValue<u32>,
    mut dropped_layer: CopyValue<bool>,
    root: Signal<Option<Rc<MountedData>>>,
    dispatch: Dispatch,
) {
    let Some(lost) = seat.and_then(|seat| seat.lost) else {
        return;
    };
    let before = *entered.read();
    spawn(async move {
        after_events().await;
        if *entered.read() != before || !host_has_focus(seat) {
            return;
        }
        // A menu or the palette of the pane's own is drawn in a layer over the window,
        // outside the root, so the focus going into one is not the host's.
        let now = dispatch.machine.state();
        let now = now.peek();
        if matches!(now.context, ContextMenu::Open { .. })
            || matches!(now.palette, PaletteState::Open { .. })
        {
            return;
        }
        if *dropped_layer.peek() {
            *dropped_layer.write() = false;
            if let Some(element) = root.peek().clone() {
                focus_soon(element);
            }
            return;
        }
        lost.call(());
    });
}

/// A recording in a pane plays in it when the host gave the edge a player; with none, it opens in
/// a viewer window of its own: the host hears it as a file to open elsewhere.
pub(super) fn use_elsewhere(hosted: bool, edge: &crate::Edge, shelf: Shelf) {
    let elsewhere = if hosted && edge.media_support() == MediaSupport::Absent {
        shelf
            .probe
            .read()
            .found()
            .filter(|probed| matches!(probed.sniffed.kind(), FormatKind::Video | FormatKind::Audio))
            .map(|probed| probed.source.path().clone())
    } else {
        None
    };
    let elsewhere_edge = edge.clone();
    use_effect(use_reactive!(|elsewhere| {
        if let Some(file) = elsewhere {
            elsewhere_edge.request(HostRequest::OpenFiles(vec![file]));
        }
    }));
}
