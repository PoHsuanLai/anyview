//! The viewer window: the root machine run with `use_machine`, its outputs carried out
//! (`carry.rs`), the results of workers fed back as inputs (`arrive.rs`), and every region drawn
//! from the machine's state. Nothing here decides: a pointer move is a chrome input, a key is a
//! `Key`, a result is a load input with its ticket, a dropped file is a `Dropped`, and what the
//! file looks like is the stage's. The window's layout rules, stage context, effects, keys and
//! sheets are in the modules below.

mod cx;
mod hooks;
mod keys;
mod layers;
mod layout;
mod parts;
mod sheets;
mod wiring;

use self::cx::stage_cx;
use self::hooks::{
    focus_left, host_has_focus, use_close_on_resize, use_elsewhere, use_focus_return, use_mailbox,
    use_pane_host,
};
use self::keys::{KeyScene, window_key};
use self::layers::{context_view, palette_view, titlebar_view};
use self::layout::{NARROW_BELOW, bar_room, capsule_shows_something, panel_pane, panel_width_in};
use self::parts::{capsule_slots, find_hits, load_state, panel_tabs, reveal, window_title};
use self::sheets::sheet_view;
use self::wiring::{Wiring, use_wiring};
use super::chrome::{BarRoom, Controls};
use super::effects::{use_announce, use_work};
use super::failed::{FailedScreen, Offer};
use super::panel::InfoPanel;
use super::press::press_of;
use super::scrub::{levelled, scrubbed};
use crate::{
    ChromeIn, Command, ContextIn, ContextMenu, HandIn, Launch, LoadFailure, NavigateIn,
    Palette as PaletteState, PaneChrome, Panel, PanelIn, PanelTab, Presentation, Sheet, Spot,
    ViewerIn, Zone,
};
use anyview_core::FilePath;
use chordkit::Context;
use dioxus::prelude::*;
use ds::components::app::edge_peek::EdgePeek;
use ds::components::chrome::split_view::model::{PaneSize, SplitPane};
use ds::components::chrome::split_view::view::SplitView;
use ds::components::chrome::titlebar_parts::DocumentState;
use ds::file_drop::hook::use_file_drop;
use ds::focus::soon::focus_soon;
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds::window::vocab::Activation;
use ds_core::vocab::ShortcutKey;
use std::rc::Rc;

/// What the viewer window draws for `launch`. A pane (`Presentation::Pane`) is the same window as
/// a region of its host's: no titlebar, welcome window, sheets, palette or file drop, none of the
/// viewer's chords, no key while the host does not give it the focus, and Esc giving the focus
/// back once there is nothing left to undo.
#[component]
pub(super) fn ViewerWindow(launch: Launch) -> Element {
    let Wiring {
        edge,
        hosted,
        seat,
        chords,
        pane_chrome,
        keymap,
        frame,
        shelf,
        area,
        measured,
        whole,
        whole_probe,
        carry,
        dispatch,
    } = use_wiring(&launch);
    // The host's focus: a pane with no seat has been given nothing to wait for.
    let heard = move || host_has_focus(seat);
    let window = use_hook(try_consume_context::<WindowHost>);
    let mut zone = use_signal(|| Zone::Content);
    let mut root = use_signal(|| None::<Rc<MountedData>>);
    // A pane hears that the focus left it for the host's other parts: the focus moving to one of
    // its own parts is a `focusin` the root hears right after the `focusout`.
    let entered = use_hook(|| CopyValue::new(0_u32));
    // A menu or the palette closing inside the pane drops the focus with it: that is the pane's
    // own layer going away, not the host taking the keyboard, so the root takes the focus back.
    let layered = use_hook(|| CopyValue::new(false));
    let mut dropped_layer = use_hook(|| CopyValue::new(false));

    // The window opens its file once it has drawn.
    let first = launch.clone();
    use_effect(move || {
        if let Some(sequence) = first.sequence.clone() {
            dispatch.send(ViewerIn::Navigate(NavigateIn::Start(sequence)));
        }
        dispatch.send(ViewerIn::Open(first.file.clone()));
    });

    use_pane_host(seat, dispatch, root, hosted);

    // A window that stops being the active one never hears Space come up, and `onblur` on the root
    // does not bubble from a child that held the focus: the host's activation is what says so.
    let activation = window.clone();
    use_effect(move || {
        if let Some(host) = &activation
            && host.state().activated == Activation::Inactive
        {
            dispatch.send(ViewerIn::Hand(HandIn::SpaceUp));
        }
    });

    use_mailbox(&edge, &carry);
    use_announce(&carry);
    use_work(&carry);

    // Files dropped on the window open, and the folder of the first is the list to walk.
    let drop = use_file_drop(move |files: ds::file_drop::drag::FileDrop| {
        let paths: Vec<FilePath> = files
            .paths
            .iter()
            .filter_map(|path| FilePath::new(path).ok())
            .collect();
        dispatch.send(ViewerIn::Dropped(paths));
    });

    let state = dispatch.machine.state()();
    let presentation = state.presentation;
    let palette_open = matches!(state.palette, PaletteState::Open { .. });
    // The keys come back to the window once a value typed into a cell or a node is taken or let go,
    // and once the palette that held them is closed.
    use_focus_return(
        root,
        shelf.edit.handle,
        dispatch,
        state.stage.is_typing(),
        palette_open,
    );

    let current = shelf.shown();
    let ticket = shelf.probe.read().ticket().unwrap_or_default();
    use_elsewhere(hosted, &carry.edge, shelf);
    let reveal_edge = carry.edge.clone();
    let cx = stage_cx(&state, dispatch, &carry.edge, ticket, area(), frame, hosted);
    let facts = current
        .as_ref()
        .map(|(_, doc)| doc.facts())
        .unwrap_or_default();
    let tabs = panel_tabs(
        current.as_ref().map(|(_, doc)| doc),
        &state,
        shelf,
        hosted,
        pane_chrome,
    );
    // Worked out again when what it reads changes, not at every render. The desktop's motion
    // level is one of those reads, through the scope signal `Level::now` peeks at.
    let scope = try_use_context::<Signal<ds::style::scope::Scope>>();
    let params_memo = use_memo(move || {
        if let Some(scope) = scope {
            std::mem::drop(scope.read());
        }
        dispatch.watched_params()
    });
    let machine_params = params_memo();
    // The capsule lists from what the palette and the menu list: a button for a file action the
    // file does not take (a rotate on a file that refuses a save) is not drawn.
    let slots = capsule_slots(
        current.as_ref().map(|(_, doc)| doc),
        &cx,
        hosted,
        &machine_params.files,
    );
    let capsule_drawn =
        !(hosted && pane_chrome == PaneChrome::Minimal) && capsule_shows_something(&slots, area());
    let (failure, phase) = load_state(state.load, current.is_some(), shelf);
    let title = window_title(shelf);
    // What the host's palette lists of this pane: every command of the file on screen, under its name.
    let offered = machine_params.palette.rows.clone();
    let named = title.clone();
    use_effect(use_reactive!(|offered, named| {
        if let Some(seat) = seat {
            seat.link.publish(named, offered);
        }
    }));
    let failed_offer = if failure == Some(LoadFailure::NotFound) {
        Offer::Nothing
    } else {
        Offer::of(
            shelf.probe.read().found().is_some() || shelf.wanted.read().is_some(),
            shelf.platform,
        )
    };
    let (panel_shown, panel_tab) = match state.panel {
        Panel::Shown { tab } => (Shown::Visible, tab),
        Panel::Hidden => (Shown::Hidden, PanelTab::Info),
    };
    let panel_tab = tabs.showing(panel_tab);
    let body = current
        .as_ref()
        .and_then(|(_, doc)| doc.view().panel(panel_tab, &cx));
    let trailing = match failure {
        Some(_) => None,
        None => current.as_ref().and_then(|(_, doc)| doc.view().modes(&cx)),
    };
    let kind = shelf
        .probe
        .read()
        .found()
        .map(|probed| probed.sniffed.kind());
    // A file with no tabs has no panel to open, whatever the machine remembers.
    let pane_shown = if tabs.first().is_some() {
        panel_shown
    } else {
        Shown::Hidden
    };
    let panel = rsx! {
        InfoPanel {
            tab: panel_tab,
            tabs,
            name: title.clone(),
            kind,
            facts,
            body,
            shown: pane_shown,
            // The titlebar lies over the pane of a window; a pane has none.
            header: (!hosted).then(|| rsx! { div { class: "viewer-panel-bar" } }),
            onchoose: move |tab: PanelTab| dispatch.send(ViewerIn::Panel(PanelIn::Choose(tab))),
        }
    };
    // A pane too narrow to share its room with the sidebar lays it over the content.
    let overlay = whole().is_some_and(|whole| whole.size.width.0 < NARROW_BELOW)
        && pane_shown == Shown::Visible;
    // How much of the titlebar the mode control takes, by the window's width.
    let bar = bar_room(whole().map(|whole| whole.size.width.0));
    let trailing = if bar == BarRoom::Bare { None } else { trailing };
    // The width the person left the panel at, read once as the window opens. A pane keeps the
    // host's behaviour: its edge has no memory.
    let panel_spec = use_hook(|| panel_pane(edge.panel_width()));
    let panel_kept = edge.clone();
    let panel_resized = move |sizes: Vec<PaneSize>| {
        if let Some(width) = panel_width_in(&sizes) {
            panel_kept.keep_panel_width(width);
        }
    };
    // Opening it again is on the view it was last on, when the file has it.
    let reopened = if panel_shown == Shown::Visible {
        panel_tab
    } else {
        tabs.showing(state.panel_last.unwrap_or(panel_tab))
    };
    // A pane dragged open or shut asks the machine for the same: opening is idempotent (the tab
    // is the one the sidebar would show), so a drag that reports it twice does not close it again.
    let panel_wanted = move |(_, shown): (usize, Shown)| {
        dispatch.send(ViewerIn::Panel(match shown {
            Shown::Visible => PanelIn::Choose(reopened),
            Shown::Hidden => PanelIn::Close,
        }));
    };
    // Folded away, the sidebar peeks from the window's edge while the pointer is there, and a
    // click on the edge opens it again.
    let pane_body = rsx! {
        EdgePeek {
            label: "Sidebar",
            pinned: pane_shown,
            onpin: move |()| dispatch.send(ViewerIn::Panel(PanelIn::Choose(reopened))),
            {panel.clone()}
        }
    };
    let (hit_lines, found) = find_hits(&state, current.as_ref().map(|(_, doc)| doc), &cx, hosted);
    let pane_hits = hit_lines.clone();
    use_effect(use_reactive!(|found, pane_hits| {
        if let Some(seat) = seat {
            seat.link.publish_hits(found, pane_hits);
        }
    }));
    let rows = machine_params.palette.rows;
    let export_facts = machine_params.sheet.export;
    let offer = machine_params.sheet.media;
    let sheet_open = !matches!(state.sheet, Sheet::Closed);
    let context_rows = machine_params.context.entries;
    let context_open = matches!(state.context, ContextMenu::Open { .. });
    // An open menu closes when the window changes size.
    use_close_on_resize(whole().map(|whole| whole.size), dispatch);
    {
        let mut layered = layered;
        let open = palette_open || context_open;
        if *layered.peek() && !open {
            *dropped_layer.write() = true;
        }
        *layered.write() = open;
    }
    // The text being edited and a form field the keyboard is in take the keys: the window hears only
    // what is meant for it.
    let editing = state.stage.edited().is_some() || state.markup.focus().is_some();
    let chrome = (shelf.chrome)();
    // The edited dot: the picture or the text has changes that are not saved.
    let document = if state.unsaved() {
        DocumentState::Edited
    } else {
        DocumentState::Saved
    };

    rsx! {
        div {
            class: "viewer",
            tabindex: "0",
            "data-hosting": if hosted { "pane" },
            "data-drop": if hosted { None } else { drop.drop_attr() },
            onmounted: move |event| {
                root.set(Some(event.data()));
                whole_probe.on_mounted(event.clone());
                if hosted {
                    // The host decides when the pane has the keyboard: it takes it now only if
                    // the host has already given it (the effect above takes it later).
                    if heard() {
                        focus_soon(event.data());
                    }
                    return;
                }
                focus_soon(event.data());
                drop.mounted(event);
            },
            onkeydown: move |event: KeyboardEvent| {
                // A pane hears keys only while the host says it has the focus.
                if !heard() {
                    return;
                }
                let scene = KeyScene {
                    palette_open,
                    context_open,
                    editing,
                    sheet_open,
                    keymap,
                    chords,
                };
                window_key(&event, scene, dispatch);
            },
            // Space coming up lets the hand go; so does the window losing the keyboard, since the
            // key-up then goes elsewhere.
            onkeyup: move |event: KeyboardEvent| {
                let space = press_of(keymap, &event, Context::Normal, chords)
                    .is_some_and(|press| press.keys() == [ShortcutKey::Space]);
                if space {
                    dispatch.send(ViewerIn::Hand(HandIn::SpaceUp));
                }
            },
            onblur: move |_| dispatch.send(ViewerIn::Hand(HandIn::SpaceUp)),
            onfocusin: move |_| {
                let mut entered = entered;
                *entered.write() += 1;
                // The focus is in the pane again, so a layer that closed earlier is settled.
                *dropped_layer.write() = false;
            },
            onfocusout: move |_| focus_left(seat, entered, dropped_layer, root, dispatch),
            // A secondary click on the content opens the context menu at the pointer; over the
            // capsule or the titlebar it is theirs, and the sidebar's own stops before here. The
            // window hears it, not the stage, so the strip at the edge of a hidden sidebar counts.
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                if zone() == Zone::Content {
                    let at = event.client_coordinates();
                    let spot = Spot { x: at.x.round() as i32, y: at.y.round() as i32 };
                    dispatch.send(ViewerIn::Context(ContextIn::Open(spot)));
                }
            },
            onpointermove: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerMoved(zone()))),
            onpointerleave: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerLeft)),
            SplitView {
                label: "Viewer",
                // A file with no views has no sidebar, and a narrow window lays it over the stage.
                panes: if tabs.first().is_some() && !overlay {
                    vec![SplitPane::new(panel_spec, pane_body).shown(pane_shown).peeking()]
                } else {
                    Vec::new()
                },
                on_shown: panel_wanted,
                on_resized: panel_resized,
                common: Common {
                    extra_class: ExtraClass::parse("viewer-split").ok(),
                    ..Common::default()
                },
                div {
                    class: "viewer-stage",
                    onmounted: move |event| measured.on_mounted(event),
                    // The small window has no frame to take hold of: a press on the picture moves it.
                    onpointerdown: move |_| {
                        if let (Presentation::Mini, Some(window)) = (presentation, window.as_ref()) {
                            window.host().begin_move();
                        }
                    },
                    if let Some(reason) = failure {
                        FailedScreen {
                            reason,
                            name: title.clone(),
                            offer: failed_offer,
                            onreveal: move |()| reveal(&reveal_edge, &shelf),
                        }
                    } else {
                        Loadable { phase,
                            if let Some((_, doc)) = current.as_ref() {
                                {doc.view().stage(&cx)}
                            }
                        }
                    }
                    if overlay {
                        div {
                            class: "viewer-panel-scrim",
                            onclick: move |_| dispatch.send(ViewerIn::Panel(PanelIn::Close)),
                        }
                        div { class: "viewer-panel-overlay", {panel} }
                    }
                    if capsule_drawn && !slots.is_empty() {
                        Controls {
                            slots,
                            shown: chrome,
                            onpick: move |command: Command| dispatch.send(ViewerIn::Run(command)),
                            onscrub: move |event| scrubbed(dispatch, event),
                            onlevel: move |at| levelled(dispatch, at),
                            onpointerenter: move |()| zone.set(Zone::Capsule),
                            onpointerleave: move |()| zone.set(Zone::Content),
                        }
                    }
                }
            }
            // The small borderless window has no titlebar: the picture is what is dragged. The bar
            // lies over the panel's pane and the stage alike.
            {titlebar_view(state.presentation, title, bar, chrome, trailing, document, zone)}
            {palette_view(&state, rows, hit_lines, found, (keymap, chords), dispatch)}
            {context_view(&state.context, context_rows, dispatch)}
            {sheet_view(&state.sheet, dispatch, shelf, &carry.edge, export_facts, &offer)}
        }
    }
}
