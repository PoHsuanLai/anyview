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
use super::export::ExportSheet;
use super::failed::{FailedScreen, Offer};
use super::palette::Palette;
use super::pane::{PaneSeat, after_events};
use super::panel::InfoPanel;
use super::press::{press_of, use_viewer_keys};
use super::resize::ResizeSheet;
use super::scrub::{levelled, scrubbed};
use super::session::{Probe, WHOLE_HITS, offered_slots, without_find};
use super::sheet::{
    EditSheet, InstallSheet, NameSheet, NoVersionsSheet, ReplaceSheet, RevertSheet, TrashSheet,
    UnavailableSheet,
};
use super::shelf::{Dispatch, Shelf, use_area, viewer_params};
use super::unsaved::UnsavedSheet;
use crate::families::FrameLook;
use crate::io::{FileAccess, HostRequest, Job, MediaSupport};
use crate::{
    Chords, ChromeIn, Command, ContextIn, ContextMenu, HandIn, Launch, Load, LoadFailure,
    NavigateIn, Palette as PaletteState, PaletteIn, PaletteScope, PaneChrome, Panel, PanelIn,
    PanelTab, PictureEditIn, PictureSheet, PictureSheetIn, Presentation, Sheet, SheetIn, Spot,
    StageCx, StageIn, TypedText, Viewer, ViewerIn, Zone,
};
use anyview_core::{FilePath, FormatKind};
use chordkit::Context;
use dioxus::prelude::*;
use ds::components::chrome::split_view::model::{Collapsing, PaneSize, PaneSpec, SplitPane};
use ds::components::chrome::split_view::view::SplitView;
use ds::components::chrome::titlebar_parts::DocumentState;
use ds::file_drop::hook::use_file_drop;
use ds::focus::soon::focus_soon;
use ds::machine::{use_machine_in, use_machine_state};
use ds::prelude::*;
use ds::root::pass_through::ExtraClass;
use ds::window::vocab::Activation;
use ds_blitz::{CloseAnswer, use_close_request, use_gpu};
use ds_core::vocab::ShortcutKey;
use ds_core::word::Word;
use futures_util::StreamExt;
use std::rc::Rc;

/// The left panel's pane: 220 wide, between 180 and 320, and it folds away when dragged past half
/// its least. It sits beside the stage, so opening it shrinks the stage and not the window.
const PANEL: PaneSpec = PaneSpec {
    preferred: PaneSize::Fixed(Px(220.0)),
    min: PaneSize::Fixed(Px(180.0)),
    max: PaneSize::Fixed(Px(320.0)),
    collapsing: Collapsing::Snaps,
};

/// Below this width the info panel of a pane lies over the content instead of beside it: opening
/// it would leave the content too little room.
const NARROW_BELOW: f32 = 480.0;

/// What the viewer window draws for `launch`. A pane (`Presentation::Pane`) is the same window as
/// a region of its host's: no titlebar, welcome window, sheets, palette or file drop, none of the
/// viewer's chords, no key while the host does not give it the focus, and Esc giving the focus
/// back once there is nothing left to undo.
#[component]
pub(super) fn ViewerWindow(launch: Launch) -> Element {
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
    // The host's focus: a pane with no seat has been given nothing to wait for.
    let heard = move || seat.is_none_or(|seat| (seat.focused)());
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
    let window = use_hook(try_consume_context::<WindowHost>);
    let mut zone = use_signal(|| Zone::Content);
    let mut root = use_signal(|| None::<Rc<MountedData>>);
    // A pane hears that the focus left it for the host's other parts: the focus moving to one of
    // its own parts is a `focusin` the root hears right after the `focusout`.
    let entered = use_hook(|| CopyValue::new(0_u32));
    let lost = seat.and_then(|seat| seat.lost);

    // The window opens its file once it has drawn.
    let first = launch.clone();
    use_effect(move || {
        if let Some(sequence) = first.sequence.clone() {
            dispatch.send(ViewerIn::Navigate(NavigateIn::Start(sequence)));
        }
        dispatch.send(ViewerIn::Open(first.file.clone()));
    });

    // The host gives the pane the keyboard: the root takes the focus, as a window's does when it
    // opens. (The pane never takes it unasked.)
    use_effect(move || {
        if hosted
            && heard()
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
        }
    });
    use_drop(move || {
        if let Some(seat) = seat {
            seat.link.withdraw();
        }
    });

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
    let palette_open = matches!(state.palette, PaletteState::Open { .. });
    // The keys belong to the window again once the palette that held them is closed.
    let editor = shelf.edit.handle;
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

    let current = shelf.shown();
    let ticket = shelf.probe.read().ticket().unwrap_or_default();
    // A recording in a pane plays in it when the host gave the edge a player; with none, it opens
    // in a viewer window of its own: the host hears it as a file to open elsewhere.
    let elsewhere = if hosted && carry.edge.media_support() == MediaSupport::Absent {
        shelf
            .probe
            .read()
            .found()
            .filter(|probed| matches!(probed.sniffed.kind(), FormatKind::Video | FormatKind::Audio))
            .map(|probed| probed.source.path().clone())
    } else {
        None
    };
    let elsewhere_edge = carry.edge.clone();
    use_effect(use_reactive!(|elsewhere| {
        if let Some(file) = elsewhere {
            elsewhere_edge.request(HostRequest::OpenFiles(vec![file]));
        }
    }));
    let worker = carry.edge.clone();
    let requester = carry.edge.clone();
    let reveal_edge = carry.edge.clone();
    let helper_edge = carry.edge.clone();
    let cx = StageCx {
        stage: state.stage.clone(),
        hand: state.hand,
        picture: state.picture.clone(),
        edit: EventHandler::new(move |input: PictureEditIn| {
            dispatch.send(ViewerIn::Picture(input));
        }),
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
        // A pane changes nothing in the file: what edits is the host's to offer.
        access: if hosted {
            FileAccess::ReadOnly
        } else {
            shelf
                .probe
                .read()
                .found()
                .map_or(FileAccess::Writable, |probed| probed.access)
        },
        text_edit: shelf.edit.session,
        text_editor: shelf.edit.handle,
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
    // A pane draws the info panel only when its host asked for it.
    let tabs = if hosted && pane_chrome != PaneChrome::WithPanel {
        crate::PanelTabs::default()
    } else {
        tabs
    };
    let machine_params = dispatch.params();
    // The capsule lists from what the palette and the menu list: a button for a file action the
    // file does not take (a rotate on a file that refuses a save) is not drawn.
    let slots = current
        .as_ref()
        .map(|(_, doc)| {
            let listed = doc.view().slots(&cx);
            // A pane has no palette for the Find button to open.
            let listed = if hosted { without_find(listed) } else { listed };
            offered_slots(listed, &machine_params.files)
        })
        .unwrap_or_default();
    let capsule_drawn = !(hosted && pane_chrome == PaneChrome::Minimal);
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
            onchoose: move |tab: PanelTab| dispatch.send(ViewerIn::Panel(PanelIn::Choose(tab))),
        }
    };
    // A pane too narrow to share its room with the panel lays the panel over the content.
    let overlay = hosted
        && whole().is_some_and(|whole| whole.size.width.0 < NARROW_BELOW)
        && pane_shown == Shown::Visible;
    // A pane dragged open or shut asks the machine for the same: opening is idempotent (the tab
    // is the one the panel would show), so a drag that reports it twice does not close it again.
    let panel_wanted = move |(_, shown): (usize, Shown)| {
        dispatch.send(ViewerIn::Panel(match shown {
            Shown::Visible => PanelIn::Choose(panel_tab),
            Shown::Hidden => PanelIn::Close,
        }));
    };
    // What the palette lists under "In This File" when it is a find.
    let hit_lines = match (&state.palette, state.palette_scope, current.as_ref()) {
        (PaletteState::Open { .. }, PaletteScope::Find(_), Some((_, doc))) => {
            doc.view().hit_lines(&cx, WHOLE_HITS)
        }
        _ => Vec::new(),
    };
    let found = state
        .stage
        .find_state()
        .and_then(|(_, hits)| hits.count())
        .map_or(0, |count| count.0);
    let rows = machine_params.palette.rows;
    let export_facts = machine_params.sheet.export;
    let offer = machine_params.sheet.media;
    let sheet_open = !matches!(state.sheet, Sheet::Closed);
    let context_rows = machine_params.context.entries;
    let context_open = matches!(state.context, ContextMenu::Open { .. });
    let keyed = state.clone();
    let editing = state.stage.edited().is_some();
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
                // The palette and the context menu take their own keys. A sheet does too, but when
                // focus is still on the window Return and Esc reach it here, so the machine's sheet
                // answers them.
                if palette_open || context_open {
                    return;
                }
                // The text being edited has the keys: the window hears only what is meant for it.
                let context = if editing {
                    Context::TextEntry
                } else {
                    Context::Normal
                };
                let Some(press) = press_of(keymap, &event, context, chords) else {
                    return;
                };
                if editing && !sheet_open && !press.for_the_window() {
                    return;
                }
                if sheet_open {
                    if SheetIn::from_press(&press).is_some() {
                        event.prevent_default();
                        dispatch.send(ViewerIn::Key(press));
                    }
                    return;
                }
                event.prevent_default();
                dispatch.send(ViewerIn::Key(press));
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
            },
            onfocusout: move |_| {
                let Some(lost) = lost else { return };
                let before = *entered.read();
                spawn(async move {
                    after_events().await;
                    if *entered.read() == before && heard() {
                        lost.call(());
                    }
                });
            },
            onpointermove: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerMoved(zone()))),
            onpointerleave: move |_| dispatch.send(ViewerIn::Chrome(ChromeIn::PointerLeft)),
            SplitView {
                label: "Viewer",
                // A folded pane leaves the split view altogether: its divider would take a strip at the
                // stage's left edge (a right-click there, a drag that starts there).
                panes: match (pane_shown, overlay) {
                    (Shown::Visible, false) => vec![SplitPane::new(PANEL, panel.clone())],
                    (Shown::Visible, true) | (Shown::Hidden, _) => Vec::new(),
                },
                on_shown: panel_wanted,
                common: Common {
                    extra_class: ExtraClass::parse("viewer-split").ok(),
                    ..Common::default()
                },
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
                    if overlay {
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
            match keyed.presentation {
                Presentation::Mini | Presentation::Pane => rsx! {},
                Presentation::Window | Presentation::Peek | Presentation::Background => rsx! {
                    Titlebar {
                        title,
                        shown: chrome,
                        trailing,
                        document,
                        onpointerenter: move |()| zone.set(Zone::Capsule),
                        onpointerleave: move |()| zone.set(Zone::Content),
                    }
                },
            }
            if let PaletteState::Open { query: typed, selection, .. } = &keyed.palette {
                Palette {
                    query: typed.clone(),
                    rows,
                    selection: *selection,
                    scope: keyed.palette_scope,
                    hits: hit_lines,
                    found,
                    ontyped: move |text: TypedText| dispatch.send(ViewerIn::Palette(PaletteIn::Typed(text))),
                    onpick: move |row| dispatch.send(ViewerIn::Palette(PaletteIn::Pick(row))),
                    onkey: move |event: KeyboardEvent| {
                        if let Some(press) = press_of(keymap, &event, Context::TextEntry, chords) {
                            dispatch.send(ViewerIn::Key(press));
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
                Sheet::ConfirmReplace => rsx! {
                    ReplaceSheet {
                        name: title_of(&shelf.probe.read()).unwrap_or_default(),
                        onreplace: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
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
                Sheet::Picture(PictureSheet::Resize(draft)) => rsx! {
                    ResizeSheet {
                        draft: *draft,
                        onchange: move |change| dispatch.send(ViewerIn::Sheet(SheetIn::Picture(PictureSheetIn::Resize(change)))),
                        onconfirm: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Unsaved(_) => rsx! {
                    UnsavedSheet {
                        name: title_of(&shelf.probe.read()).unwrap_or_default(),
                        onsave: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Confirm)),
                        ondiscard: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Discard)),
                        oncancel: move |()| dispatch.send(ViewerIn::Sheet(SheetIn::Cancel)),
                    }
                },
                Sheet::Export { draft, span } => rsx! {
                    ExportSheet {
                        draft: *draft,
                        span: *span,
                        facts: export_facts,
                        offer: offer.clone(),
                        name: title_of(&shelf.probe.read()).unwrap_or_default(),
                        onpick: move |pick| dispatch.send(ViewerIn::Sheet(SheetIn::PickKind(pick))),
                        ontune: move |option| dispatch.send(ViewerIn::Sheet(SheetIn::Tune(option))),
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

/// Show the file the window last asked for in its folder.
fn reveal(edge: &crate::Edge, shelf: &Shelf) {
    if let Some(file) = shelf.wanted.peek().clone() {
        edge.request(HostRequest::Reveal(file));
    }
}
