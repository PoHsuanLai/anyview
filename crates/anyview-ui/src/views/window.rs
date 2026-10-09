//! The viewer window: the root machine run with `use_machine`, its outputs carried out
//! (`carry.rs`), the results of workers fed back as inputs (`arrive.rs`), and every region drawn
//! from the machine's state. Nothing here decides: a pointer move is a chrome input, a key is a
//! `Key`, a result is a load input with its ticket, a dropped file is a `Dropped`, and what the
//! file looks like is the stage's.

use super::arrive::arrived;
use super::carry::{Carry, carry_out};
use super::chrome::{Controls, Titlebar};
use super::context::ContextPopup;
use super::effects::{use_announce, use_work};
use super::failed::{FailedScreen, Offer};
use super::keys::{keys_of, shortcut_of};
use super::palette::Palette;
use super::panel::InfoPanel;
use super::scrub::{levelled, scrubbed};
use super::session::{Probe, offered_slots};
use super::sheet::{
    EditSheet, ExportSheet, InstallSheet, NameSheet, NoVersionsSheet, RevertSheet, TrashSheet,
    UnavailableSheet,
};
use super::shelf::{Dispatch, Shelf, use_area, viewer_params};
use crate::families::FrameLook;
use crate::io::{HostRequest, Job};
use crate::{
    ChromeIn, Command, ContextIn, ContextMenu, Launch, Load, LoadFailure, NavigateIn,
    Palette as PaletteState, PaletteIn, Panel, PanelIn, PanelTab, Presentation, Sheet, SheetIn,
    Spot, StageCommand, StageCx, StageIn, TypedText, Viewer, ViewerIn, Zone,
};
use anyview_core::FilePath;
use dioxus::prelude::*;
use ds::file_drop::hook::use_file_drop;
use ds::focus::soon::focus_soon;
use ds::machine::{use_machine_in, use_machine_state};
use ds::prelude::*;
use ds_blitz::use_gpu;
use ds_core::vocab::ShortcutKey;
use ds_core::word::Word;
use futures_util::StreamExt;
use std::rc::Rc;

/// What the viewer window draws for `launch`.
#[component]
pub(super) fn ViewerWindow(launch: Launch) -> Element {
    let edge = use_hook(consume_context::<crate::Edge>);
    let gpu = use_gpu();
    let scope = use_scope();
    let scale = try_consume_context::<HostSignals>().map_or(Scale::ONE, |host| (host.scale)());
    let shelf = Shelf::empty(edge.platform());
    let (area, measured) = use_area(scale);
    let slot = use_hook(|| CopyValue::new(None));
    let carry = Carry {
        shelf,
        edge: edge.clone(),
        gpu: gpu.clone(),
        toasts: ds::prelude::use_toasts(),
        machine: slot,
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
    let window = use_hook(try_consume_context::<WindowHost>);
    let mut zone = use_signal(|| Zone::Content);
    let mut root = use_signal(|| None::<Rc<MountedData>>);

    // The window opens its file once it has drawn.
    let first = launch.clone();
    use_effect(move || {
        if let Some(sequence) = first.sequence.clone() {
            dispatch.send(ViewerIn::Navigate(NavigateIn::Start(sequence)));
        }
        dispatch.send(ViewerIn::Open(first.file.clone()));
    });

    // What workers made comes back through the mailbox, one input at a time.
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

    let state = machine.state()();
    let presentation = state.presentation;
    let finding = state.stage.is_finding();
    // The keys belong to the window again once the find that held them is closed.
    use_effect(use_reactive!(|finding| {
        if !finding && let Some(element) = root.peek().clone() {
            focus_soon(element);
        }
    }));

    let current = shelf.shown();
    let ticket = shelf.probe.read().ticket().unwrap_or_default();
    let typing_in = dispatch;
    let worker = carry.edge.clone();
    let requester = carry.edge.clone();
    let reveal_edge = carry.edge.clone();
    let helper_edge = carry.edge.clone();
    let cx = StageCx {
        stage: state.stage.clone(),
        ticket,
        area: area(),
        send: EventHandler::new(move |input: StageIn| dispatch.send(ViewerIn::Stage(input))),
        run: EventHandler::new(move |command: Command| dispatch.send(ViewerIn::Run(command))),
        lines: (shelf.lines)(),
        ask_lines: EventHandler::new(move |(first, rows): (anyview_core::LineIndex, u32)| {
            if let Some((held, doc)) = shelf.shown_now()
                && let Some(job) = doc.view().lines(held, first, rows)
            {
                carry.edge.submit(job);
            }
        }),
        typing: EventHandler::new(move |event: KeyboardEvent| typed(typing_in, &event)),
        hits: (shelf.hits)(),
        work: EventHandler::new(move |job: Job| worker.submit(job)),
        request: EventHandler::new(move |request: HostRequest| requester.request(request)),
        pdf: shelf.pdf,
        media: shelf.media,
        frame: FrameLook {
            attributes: format!(
                "data-theme=\"{}\" data-accent=\"{}\" data-motion=\"{}\" data-material=\"window\"",
                scope.scheme.slug(),
                scope.resolved.accent.slug(),
                scope.resolved.motion.slug(),
            ),
        },
        platform: shelf.platform,
    };
    let facts = current
        .as_ref()
        .map(|(_, doc)| doc.facts())
        .unwrap_or_default();
    let tabs = current.as_ref().map_or_else(Default::default, |(_, doc)| {
        let tabs = doc.view().panel_params().tabs;
        if matches!(state.stage, crate::Stage::Media(_)) {
            crate::families::media_tabs(tabs, shelf.media.read().abilities)
        } else {
            tabs
        }
    });
    let machine_params = dispatch.params();
    // The capsule lists from what the palette and the menu list: a button for a file action the
    // file does not take (a rotate on a file that refuses a save) is not drawn.
    let slots = current
        .as_ref()
        .map(|(_, doc)| offered_slots(doc.view().slots(&cx), &machine_params.files))
        .unwrap_or_default();
    let failure = match state.load {
        Load::Failed { reason, .. } => Some(reason),
        Load::Idle { .. }
        | Load::Probing { .. }
        | Load::Peeking { .. }
        | Load::Opening { .. }
        | Load::Ready { .. } => None,
    };
    let phase = match state.load {
        Load::Failed { .. } | Load::Ready { .. } => Phase::Ready,
        Load::Idle { .. } | Load::Probing { .. } | Load::Peeking { .. } | Load::Opening { .. } => {
            if current.is_some() {
                Phase::Ready
            } else {
                Phase::Loading((shelf.operation)())
            }
        }
    };
    let title = title_of(&shelf.probe.read()).unwrap_or_else(|| {
        (shelf.wanted)()
            .as_ref()
            .and_then(FilePath::file_name)
            .map_or_else(String::new, |name| name.as_str().to_owned())
    });
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
    let rows = machine_params.palette.rows;
    let offer = machine_params.sheet.media;
    let sheet_open = !matches!(state.sheet, Sheet::Closed);
    let palette_open = matches!(state.palette, PaletteState::Open { .. });
    let context_rows = machine_params.context.entries;
    let context_open = matches!(state.context, ContextMenu::Open { .. });
    let keyed = state.clone();
    let chrome = (shelf.chrome)();

    rsx! {
        div {
            class: "viewer",
            tabindex: "0",
            "data-drop": drop.drop_attr(),
            onmounted: move |event| {
                root.set(Some(event.data()));
                focus_soon(event.data());
                drop.mounted(event);
            },
            onkeydown: move |event: KeyboardEvent| {
                // The palette and the context menu take their own keys. A sheet does too, but when
                // focus is still on the window Return and Esc reach it here, so the machine's sheet
                // answers them.
                if palette_open || context_open {
                    return;
                }
                if sheet_open {
                    if let Some(key) = shortcut_of(&event)
                        && SheetIn::from_key(&key.keys()).is_some()
                    {
                        event.prevent_default();
                        dispatch.send(ViewerIn::Key(key));
                    }
                    return;
                }
                if let Some(key) = shortcut_of(&event) {
                    event.prevent_default();
                    dispatch.send(ViewerIn::Key(key));
                }
            },
            onpointermove: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerMoved(zone()))),
            onpointerleave: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerLeft)),
            div {
                class: "viewer-stage",
                onmounted: move |event| measured.on_mounted(event),
                // A secondary click on the content opens the context menu at the pointer; over the
                // capsule or the titlebar it is theirs.
                oncontextmenu: move |event: MouseEvent| {
                    event.prevent_default();
                    if zone() == Zone::Content {
                        let at = event.client_coordinates();
                        let spot = Spot { x: at.x.round() as i32, y: at.y.round() as i32 };
                        dispatch.send(ViewerIn::Context(ContextIn::Open(spot)));
                    }
                },
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
                if !slots.is_empty() {
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
                // The small borderless window has no titlebar: the picture is what is dragged.
                match keyed.presentation {
                    Presentation::Mini => rsx! {},
                    Presentation::Window | Presentation::Peek | Presentation::Background => rsx! {
                        Titlebar {
                            title,
                            shown: chrome,
                            onpointerenter: move |()| zone.set(Zone::Capsule),
                            onpointerleave: move |()| zone.set(Zone::Content),
                        }
                    },
                }
            }
            InfoPanel {
                shown: panel_shown,
                tab: panel_tab,
                tabs,
                facts,
                body,
                onchoose: move |tab: PanelTab| dispatch.send(ViewerIn::Panel(PanelIn::Choose(tab))),
                onclose: move |()| dispatch.send(ViewerIn::Panel(PanelIn::Close)),
            }
            if let PaletteState::Open { query: typed, selection } = &keyed.palette {
                Palette {
                    query: typed.clone(),
                    rows,
                    selection: *selection,
                    ontyped: move |text: TypedText| {
                        let mut query = shelf.query;
                        query.set(text.clone());
                        dispatch.send(ViewerIn::Palette(PaletteIn::Typed(text)));
                    },
                    onpick: move |row| dispatch.send(ViewerIn::Palette(PaletteIn::Pick(row))),
                    onkey: move |event: KeyboardEvent| {
                        if let Some(key) = shortcut_of(&event) {
                            dispatch.send(ViewerIn::Key(key));
                        }
                    },
                    onclose: move |()| dispatch.send(ViewerIn::Palette(PaletteIn::Close)),
                }
            }
            if let ContextMenu::Open { at } = keyed.context {
                ContextPopup {
                    entries: context_rows.clone(),
                    at,
                    onpick: move |pick| dispatch.send(ViewerIn::Context(ContextIn::Pick(pick))),
                    onclose: move |()| dispatch.send(ViewerIn::Context(ContextIn::Close)),
                }
            }
            match &keyed.sheet {
                Sheet::Closed => rsx! {},
                Sheet::ConfirmTrash => rsx! {
                    TrashSheet {
                        name: title_of(&shelf.probe.read()).unwrap_or_default(),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::ConfirmEdit { request, caution } => rsx! {
                    EditSheet {
                        edit: request.edit,
                        caution: *caution,
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Rename { name } => rsx! {
                    NameSheet {
                        label: "Rename",
                        confirm: "Rename",
                        name: name.clone(),
                        ontyped: move |text: TypedText| dispatch.send(ViewerIn::Sheet(SheetIn::Typed(text))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::SaveCopy { name } => rsx! {
                    NameSheet {
                        label: "Save a Copy",
                        confirm: "Save",
                        name: name.clone(),
                        ontyped: move |text: TypedText| dispatch.send(ViewerIn::Sheet(SheetIn::Typed(text))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Revert { versions, chosen } => rsx! {
                    RevertSheet {
                        versions: versions.clone(),
                        chosen: chosen.clone(),
                        onpick: move |key| dispatch.send(ViewerIn::Sheet(SheetIn::PickVersion(key))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::NoVersions => rsx! {
                    NoVersionsSheet {
                        onclose: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Unavailable { needs, helper } => rsx! {
                    UnavailableSheet {
                        needs: needs.clone(),
                        helper: *helper,
                        onclose: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                        oninstall: move |helper| dispatch.send(ViewerIn::Sheet(SheetIn::OfferHelper(helper))),
                    }
                },
                Sheet::Helper { helper, phase } => match helper_edge.helper_words(*helper) {
                    Some(words) => rsx! {
                        InstallSheet {
                            words,
                            phase: phase.clone(),
                            oninstall: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                            ondismiss: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                        }
                    },
                    None => rsx! {},
                },
                Sheet::Export { draft } => rsx! {
                    ExportSheet {
                        draft: *draft,
                        offer: offer.clone(),
                        onpick: move |pick| dispatch.send(ViewerIn::Sheet(SheetIn::PickKind(pick))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
            }
        }
    }
}

/// The file's name, for a question about it.
fn title_of(probe: &Probe) -> Option<String> {
    probe
        .found()
        .and_then(|probed| probed.source.path().file_name())
        .map(|name| name.as_str().to_owned())
}

/// A key pressed in a field the stage drew (its find bar). Enter and Shift+Enter step through the
/// hits and Esc closes the find, as the commands and the Esc chord mean; every other key without
/// the command key is the field's own (typing), and goes no further. The command key's chords
/// (⌘G, ⌘F) go on to the window's routing.
fn typed(dispatch: Dispatch, event: &KeyboardEvent) {
    let keys = keys_of(event);
    match keys.as_slice() {
        [ShortcutKey::Enter] => {
            event.stop_propagation();
            dispatch.send(ViewerIn::Run(Command::Stage(StageCommand::FindNext)));
        }
        [ShortcutKey::Shift, ShortcutKey::Enter] => {
            event.stop_propagation();
            dispatch.send(ViewerIn::Run(Command::Stage(StageCommand::FindPrevious)));
        }
        [ShortcutKey::Escape] => {
            event.stop_propagation();
            dispatch.send(ViewerIn::Key(ds_core::vocab::Shortcut(keys)));
        }
        chord if chord.contains(&ShortcutKey::Super) => {}
        _ => event.stop_propagation(),
    }
}

/// Show the file the window last asked for in its folder.
fn reveal(edge: &crate::Edge, shelf: &Shelf) {
    if let Some(file) = shelf.wanted.peek().clone() {
        edge.request(HostRequest::Reveal(file));
    }
}
