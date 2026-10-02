//! The viewer window: the root machine run with `use_machine`, its outputs carried out, the
//! results of workers fed back as inputs, and every region drawn from the machine's state.
//! Nothing here decides: a pointer move is a chrome input, a key is a `Key`, a result is a load
//! input with its ticket, and what the file looks like is the stage's. The look is Proposal 1:
//! only the content shows by default, the titlebar and the capsule come with the pointer, ⌘K
//! opens the palette and the side panel waits to be asked for.

use super::chrome::{Controls, Titlebar};
use super::keys::shortcut_of;
use super::palette::Palette;
use super::panel::InfoPanel;
use super::session::{Probe, family, params};
use super::sheet::{ExportSheet, RenameSheet, TrashSheet};
use crate::families::{Area, FrameLook, Held, LineWindow, LoadedDoc};
use crate::io::{Done, Edge, HostRequest, Job, OpenLink};
use crate::{
    ChromeIn, ChromeOut, Command, Load, LoadFlow, LoadIn, LoadOut, NavigateIn,
    Palette as PaletteState, PaletteIn, Panel, PanelIn, PanelOut, PanelTab, PresentationOut,
    RasterOut, Sheet, SheetIn, SheetOut, StageCx, StageFamily, StageIn, StageOut, TextOut, Ticket,
    TypedText, Viewer, ViewerIn, ViewerOut, Zone,
};
use anyview_core::{FilePath, Sequence};
use dioxus::prelude::*;
use ds::focus::soon::focus_soon;
use ds::host::measure::use_rect;
use ds::machine::{MachineRef, use_machine};
use ds::motion::detail::operation::{Operation, PendingToken};
use ds::prelude::*;
use ds_blitz::use_gpu;
use ds_core::word::Word;
use futures_util::StreamExt;
use std::sync::Arc;

/// What the window was opened with: the file, and the list the arrow keys walk, when there is one.
/// The binary provides it as a root context (`ds_blitz::AppConfig::with_context`).
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    /// The file to open.
    pub file: FilePath,
    /// The files around it, with the file's place among them.
    pub sequence: Option<Sequence>,
    /// How the window looks: theme, accent, motion.
    pub appearance: Appearance,
}

/// The viewer's own stylesheet, in the `app` layer; tokens only.
pub fn stylesheet() -> String {
    [include_str!("style.css"), crate::families::TOKEN_CSS].join("\n")
}

/// The root of a viewer window: a quire `Ds` root, the stylesheet, and the window.
#[component]
pub fn ViewerApp() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    rsx! {
        Ds { appearance: launch.appearance, material: Material::Window,
            AppStyle { css: stylesheet() }
            ViewerWindow { launch: launch.clone() }
        }
    }
}

/// Sends an input to the root machine with the parameters its state calls for right now.
#[derive(Clone, Copy)]
struct Dispatch {
    machine: MachineRef<Viewer>,
    loaded: Signal<Option<(Ticket, LoadedDoc)>>,
    probe: Signal<Probe>,
    area: Memo<Option<Area>>,
    query: Signal<TypedText>,
}

impl Dispatch {
    fn params(&self) -> crate::ViewerParams {
        let state = self.machine.state().peek().clone();
        let doc = self.loaded.peek().as_ref().map(|(_, doc)| doc.clone());
        params(
            &state.stage,
            doc.as_ref(),
            &self.probe.peek(),
            *self.area.peek(),
            &self.query.peek(),
        )
    }

    fn send(&self, input: ViewerIn) {
        self.machine.set_params(self.params());
        self.machine.send(input);
    }
}

#[component]
fn ViewerWindow(launch: Launch) -> Element {
    let edge = use_hook(consume_context::<Edge>);
    let gpu = use_gpu();
    let texture = use_hook(|| gpu.handle());
    let scope = use_scope();
    let scale = try_consume_context::<HostSignals>().map_or(Scale::ONE, |host| (host.scale)());
    let mut probe = use_signal(|| Probe::Idle);
    let mut opening = use_signal(|| None::<Ticket>);
    let mut loaded = use_signal(|| None::<(Ticket, LoadedDoc)>);
    let mut lines = use_signal(|| None::<Held<LineWindow>>);
    let mut chrome_shown = use_signal(|| Shown::Hidden);
    let mut zone = use_signal(|| Zone::Content);
    let mut query = use_signal(TypedText::default);
    let mut operation = use_signal(|| Operation::Idle);
    let measured = use_rect();
    let area = use_memo(move || {
        measured.rect().map(|rect| Area {
            origin: rect.origin,
            size: rect.size,
            scale: scale.0 as f32 / Scale::DENOMINATOR as f32,
        })
    });

    let effects = edge.clone();
    let machine = use_machine::<Viewer>(
        params(
            &crate::Stage::NoStage,
            None,
            &Probe::Idle,
            None,
            &TypedText::EMPTY,
        ),
        move |out| match out {
            ViewerOut::Probe { ticket, path } => {
                probe.set(Probe::Pending(ticket));
                loaded.set(None);
                lines.set(None);
                operation.set(Operation::Running(PendingToken::start()));
                effects.submit(Job::Probe { ticket, path });
            }
            ViewerOut::Load(LoadOut::Open(ticket)) => opening.set(Some(ticket)),
            ViewerOut::Reload { .. } | ViewerOut::ListFolder(_) => {}
            ViewerOut::Load(
                LoadOut::Peek(_)
                | LoadOut::Cancel(_)
                | LoadOut::UseStage(_)
                | LoadOut::ShowFirstFrame(_)
                | LoadOut::ShowFull(_)
                | LoadOut::Probe(_),
            ) => {}
            ViewerOut::Chrome(ChromeOut::Fade { to, .. }) => chrome_shown.set(to),
            ViewerOut::Palette(crate::PaletteOut::Opened) => query.set(TypedText::EMPTY),
            ViewerOut::Palette(crate::PaletteOut::Closed | crate::PaletteOut::Run(_))
            | ViewerOut::Panel(PanelOut::Show(_) | PanelOut::Hide)
            | ViewerOut::Sheet(SheetOut::Opened | SheetOut::Closed)
            | ViewerOut::Preload(_)
            | ViewerOut::Stage(
                StageOut::Raster(RasterOut::Turned(_) | RasterOut::ShowFrame(_))
                | StageOut::Text(TextOut::ScrollTo(_) | TextOut::Show(_) | TextOut::Find(_))
                | StageOut::Pdf(_)
                | StageOut::Media(_),
            ) => {}
            ViewerOut::Stage(StageOut::Raster(RasterOut::Remember(resume))) => {
                effects.request(HostRequest::Remember(resume));
            }
            ViewerOut::Stage(StageOut::Text(TextOut::Remember(resume))) => {
                effects.request(HostRequest::Remember(resume));
            }
            ViewerOut::Sheet(SheetOut::Export(draft)) => {
                effects.request(HostRequest::Export(draft))
            }
            ViewerOut::Sheet(SheetOut::Trash) => effects.request(HostRequest::Trash),
            ViewerOut::Sheet(SheetOut::Rename(name)) => effects.request(HostRequest::Rename(name)),
            ViewerOut::Presentation(PresentationOut::Become(presentation)) => {
                effects.request(HostRequest::Present(presentation));
            }
            ViewerOut::Run(action) => effects.request(HostRequest::Run(action)),
            ViewerOut::PickFile => effects.request(HostRequest::PickFile),
            ViewerOut::CloseWindow => effects.request(HostRequest::CloseWindow),
        },
    );
    let dispatch = Dispatch {
        machine,
        loaded,
        probe,
        area,
        query,
    };

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
    use_future(move || {
        let taken = mailbox.take_mailbox();
        async move {
            let Some(mut inbox) = taken else { return };
            while let Some(done) = inbox.next().await {
                arrived(done, dispatch, &mut probe, &mut loaded, &mut lines);
            }
        }
    });

    // A probe is announced to the load machine only after a draw that already knew its kind.
    use_effect(move || {
        if let Probe::Arrived(ticket, probed) = probe() {
            let stage = family(&probed);
            probe.set(Probe::Announced(ticket, probed));
            dispatch.send(ViewerIn::Load(LoadIn::Probed {
                ticket,
                flow: LoadFlow::OpenOnly,
                stage,
            }));
        }
    });

    // A picture waits for the window's device, then opens: the worker uploads into the texture.
    let ready = gpu.device().is_some();
    let opener = edge.clone();
    let highlighter = edge.highlighter();
    use_effect(use_reactive!(|ready| {
        let Some(ticket) = opening() else { return };
        let Probe::Announced(held, probed) = probe() else {
            return;
        };
        if held != ticket || (probed.family == StageFamily::Raster && !ready) {
            return;
        }
        opening.set(None);
        opener.submit(Job::Open {
            ticket,
            probed,
            link: OpenLink {
                texture: texture.clone(),
                highlighter: Arc::clone(&highlighter),
            },
        });
    }));

    let state = machine.state()();
    let current = loaded();
    let ticket = probe().ticket().unwrap_or_default();
    let reader = edge.clone();
    let cx = StageCx {
        stage: state.stage.clone(),
        ticket,
        area: area(),
        send: EventHandler::new(move |input: StageIn| dispatch.send(ViewerIn::Stage(input))),
        run: EventHandler::new(move |command: Command| dispatch.send(ViewerIn::Run(command))),
        lines: lines(),
        ask_lines: EventHandler::new(move |(first, rows): (anyview_core::LineIndex, u32)| {
            if let Some((ticket, doc)) = loaded.peek().as_ref()
                && let Some(job) = doc.view().lines(*ticket, first, rows)
            {
                reader.submit(job);
            }
        }),
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
            Phase::Loading(operation())
        }
    };
    let title = probe()
        .found()
        .and_then(|probed| probed.source.path().file_name())
        .map_or_else(
            || {
                launch
                    .file
                    .file_name()
                    .map_or_else(String::new, |name| name.as_str().to_owned())
            },
            |name| name.as_str().to_owned(),
        );
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

    rsx! {
        div {
            class: "viewer",
            tabindex: "0",
            onmounted: move |event| focus_soon(event.data()),
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
                        shown: chrome_shown(),
                        onpick: move |command: Command| dispatch.send(ViewerIn::Run(command)),
                        onpointerenter: move |()| zone.set(Zone::Capsule),
                        onpointerleave: move |()| zone.set(Zone::Content),
                    }
                }
                Titlebar {
                    title,
                    shown: chrome_shown(),
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
                        name: title_of(&probe()),
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
fn title_of(probe: &Probe) -> String {
    probe
        .found()
        .and_then(|probed| probed.source.path().file_name())
        .map_or_else(String::new, |name| name.as_str().to_owned())
}

/// A worker's result as an input: a result for a file the person left is dropped by its ticket.
fn arrived(
    done: Done,
    dispatch: Dispatch,
    probe: &mut Signal<Probe>,
    loaded: &mut Signal<Option<(Ticket, LoadedDoc)>>,
    lines: &mut Signal<Option<Held<LineWindow>>>,
) {
    match done {
        Done::Probed { ticket, result } => match result {
            Ok(probed) if probe.peek().ticket() == Some(ticket) => {
                probe.set(Probe::Arrived(ticket, probed));
            }
            Ok(_) => {}
            Err(error) => dispatch.send(ViewerIn::Load(LoadIn::Failed {
                ticket,
                reason: error.failure(),
            })),
        },
        Done::Opened { ticket, result } => match result {
            Ok(doc) if probe.peek().ticket() == Some(ticket) => {
                loaded.set(Some((ticket, doc)));
                dispatch.send(ViewerIn::Load(LoadIn::Opened { ticket }));
            }
            Ok(_) => {}
            Err(error) => dispatch.send(ViewerIn::Load(LoadIn::Failed {
                ticket,
                reason: error.failure(),
            })),
        },
        Done::Lines { ticket, result } => {
            let current = loaded.peek().as_ref().map(|(held, _)| *held);
            if let (Some(held), Ok(window)) = (current, result)
                && held == ticket
            {
                lines.set(Some(Held(Arc::new(window))));
            }
        }
    }
}
