//! The viewer window: the root machine run with `use_machine`, its outputs carried out
//! (`carry.rs`), the results of workers fed back as inputs (`arrive.rs`), and every region drawn
//! from the machine's state. Nothing here decides: a pointer move is a chrome input, a key is a
//! `Key`, a result is a load input with its ticket, a dropped file is a `Dropped`, and what the
//! file looks like is the stage's.

use super::arrive::arrived;
use super::carry::{Carry, carry_out};
use super::chrome::{Controls, Titlebar};
use super::effects::{use_announce, use_work};
use super::keys::{keys_of, shortcut_of};
use super::palette::Palette;
use super::panel::InfoPanel;
use super::session::{Probe, params};
use super::sheet::{ExportSheet, RenameSheet, TrashSheet};
use super::shelf::{Dispatch, Shelf, use_area};
use crate::families::FrameLook;
use crate::{
    ChromeIn, Command, Launch, Load, NavigateIn, Palette as PaletteState, PaletteIn, Panel,
    PanelIn, PanelTab, Sheet, SheetIn, StageCommand, StageCx, StageIn, TypedText, Viewer, ViewerIn,
    Zone,
};
use anyview_core::FilePath;
use dioxus::prelude::*;
use ds::file_drop::hook::use_file_drop;
use ds::focus::soon::focus_soon;
use ds::machine::use_machine;
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
    let shelf = Shelf::empty();
    let (area, measured) = use_area(scale);
    let slot = use_hook(|| CopyValue::new(None));
    let carry = Carry {
        shelf,
        edge: edge.clone(),
        gpu: gpu.clone(),
        machine: slot,
    };
    let handler = carry.clone();
    let machine = use_machine::<Viewer>(
        params(
            &crate::Stage::NoStage,
            None,
            &Probe::Idle,
            None,
            &TypedText::EMPTY,
            None,
        ),
        move |out| carry_out(out, &handler),
    );
    let dispatch = Dispatch {
        machine,
        shelf,
        area,
    };
    let mut slot = slot;
    slot.set(Some(dispatch));
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
        frame: FrameLook {
            attributes: format!(
                "data-theme=\"{}\" data-accent=\"{}\" data-motion=\"{}\" data-material=\"window\"",
                scope.scheme.slug(),
                scope.resolved.accent.slug(),
                scope.resolved.motion.slug(),
            ),
        },
    };
    let facts = current
        .as_ref()
        .map(|(_, doc)| doc.view().facts())
        .unwrap_or_default();
    let tabs = current
        .as_ref()
        .map_or_else(Default::default, |(_, doc)| doc.view().panel_params().tabs);
    let slots = current
        .as_ref()
        .map(|(_, doc)| doc.view().slots(&cx))
        .unwrap_or_default();
    let phase = match state.load {
        Load::Failed { reason, .. } => Phase::Failed {
            title: "This file did not open".to_owned(),
            description: Some(reason.label().into()),
        },
        Load::Ready { .. } => Phase::Ready,
        Load::Idle { .. } | Load::Probing { .. } | Load::Peeking { .. } | Load::Opening { .. } => {
            if current.is_some() {
                Phase::Ready
            } else {
                Phase::Loading((shelf.operation)())
            }
        }
    };
    let title = title_of(&shelf.probe.read()).unwrap_or_else(|| {
        launch
            .file
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned())
    });
    let (panel_shown, panel_tab) = match state.panel {
        Panel::Shown { tab } => (Shown::Visible, tab),
        Panel::Hidden => (Shown::Hidden, PanelTab::Info),
    };
    let body = current
        .as_ref()
        .and_then(|(_, doc)| doc.view().panel(panel_tab, &cx));
    let rows = dispatch.params().palette.rows;
    let sheet_open = !matches!(state.sheet, Sheet::Closed);
    let palette_open = matches!(state.palette, PaletteState::Open { .. });
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
                // A sheet and the palette take their own keys; the rest are the machine's.
                if sheet_open || palette_open {
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
                Loadable { phase,
                    if let Some((_, doc)) = current.as_ref() {
                        {doc.view().stage(&cx)}
                    }
                }
                if !slots.is_empty() {
                    Controls {
                        slots,
                        shown: chrome,
                        onpick: move |command: Command| dispatch.send(ViewerIn::Run(command)),
                        onpointerenter: move |()| zone.set(Zone::Capsule),
                        onpointerleave: move |()| zone.set(Zone::Content),
                    }
                }
                Titlebar {
                    title,
                    shown: chrome,
                    onpointerenter: move |()| zone.set(Zone::Capsule),
                    onpointerleave: move |()| zone.set(Zone::Content),
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
            match &keyed.sheet {
                Sheet::Closed => rsx! {},
                Sheet::ConfirmTrash => rsx! {
                    TrashSheet {
                        name: title_of(&shelf.probe.read()).unwrap_or_default(),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Rename { name } => rsx! {
                    RenameSheet {
                        name: name.clone(),
                        ontyped: move |text: TypedText| dispatch.send(ViewerIn::Sheet(SheetIn::Typed(text))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Export { draft } => rsx! {
                    ExportSheet {
                        draft: *draft,
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
        [ShortcutKey::Super, ..] => {}
        _ => event.stop_propagation(),
    }
}
