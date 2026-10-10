//! The root components. Every window is a [`Window`]: the program opens each through ds-blitz's
//! `AppHandle` with its [`Seed`] as props ([`window_root`]), and a harness test gives the
//! [`Seed`] as a context ([`seeded_root`]).

use super::fit::{WindowFit, natural_for, window_for};
use super::opening::Opening;
use super::seed::{Seed, StackingAsk};
use super::welcome::open_each;
use crate::host::{
    Carry, Declined, Doing, HandedResume, Listening, Outcome, Shown, StoreLocks, WindowTask,
    WindowWatch, route, subject_of, tell, tell_declined, tell_problem,
};
use anyview_core::{FilePath, Resume};
use anyview_platform::{Stacking, StackingOutcome};
use anyview_ui::{
    Edge, HelperSource, HostRequest, Launch, Notice, Presentation, ResumeSource, SaveEnd, Services,
    ViewerApp,
};
use dioxus::prelude::*;
use ds::prelude::WindowHost;
use ds_blitz::{
    AppEnded, AppHandle, Decorations, Extent, ScreenArea, WindowHandle, WindowSize, WindowSizer,
    WindowSpec, clipboard, use_window_handle, use_window_sizer,
};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// The small window of a recording: 480 by 270, a sixteenth by nine picture.
const MINI: WindowSize = WindowSize::new(480, 270);

/// The root of a window given its [`Seed`] as a context
/// (`ds_blitz::AppConfig::with_context`, or a harness's).
pub fn seeded_root() -> Element {
    let seed = use_hook(consume_context::<Seed>);
    rsx! { Window { seed } }
}

/// The root of a window opened with its [`Seed`] as props.
fn window_root(seed: Seed) -> Element {
    rsx! { Window { seed } }
}

/// Open a window on `seed`'s file, from any thread: the way a request that arrives on the
/// program's runtime reaches the event loop. It fails once the app has ended.
pub fn open_in_window(app: &AppHandle, seed: Seed) -> Result<(), AppEnded> {
    let spec = spec_for(&seed, app.screen_area());
    app.open_window_with(spec, window_root, seed)
}

/// The window a seed asks for on a `screen`: a normal one, sized to its file's content (see
/// [`window_for`]), or the small borderless one whose capsule is its only frame (the window draws
/// nothing of its own on it).
pub(super) fn spec_for(seed: &Seed, screen: Option<ScreenArea>) -> WindowSpec {
    let title = title_of(&seed.opening.file);
    match seed.presentation {
        Presentation::Mini => WindowSpec::new(title, MINI).with_decorations(Decorations::Client),
        Presentation::Window | Presentation::Peek | Presentation::Background => WindowSpec::new(
            title,
            window_for(&seed.opening.file, screen, seed.factory.window_screen),
        ),
    }
}

fn title_of(file: &FilePath) -> String {
    file.file_name().map_or_else(
        || anyview_core::APP_NAME.to_owned(),
        |name| name.as_str().to_owned(),
    )
}

/// The window's end of its requests: the receiver its [`Edge`] sends to.
#[derive(Clone)]
struct Wiring {
    edge: Edge,
    launch: Launch,
    requests: Rc<RefCell<Option<UnboundedReceiver<HostRequest>>>>,
    /// The way back into the request queue: what waited for a save is asked again through it.
    again: UnboundedSender<HostRequest>,
}

impl Wiring {
    fn new(seed: &Seed) -> Wiring {
        let (send, receive) = unbounded();
        let again = send.clone();
        let services = Services::new(Arc::clone(&seed.factory.workers), move |request| {
            // A window that closed has no receiver, and nobody is left to ask.
            let _gone = send.unbounded_send(request);
        })
        .with_resume_source(resume_of(seed))
        .with_version_source(Arc::clone(&seed.factory.versions))
        .with_first_frames(Arc::clone(&seed.factory.first_frames))
        .with_media(Arc::clone(&seed.factory.media))
        .with_image_plugins(Arc::clone(&seed.factory.image_plugins))
        .with_locks(Arc::new(StoreLocks))
        .with_platform(seed.factory.hosting.abilities());
        let services = match &seed.factory.helpers {
            Some(helpers) => services.with_helpers(Arc::clone(helpers) as Arc<dyn HelperSource>),
            None => services,
        };
        let edge = Edge::new(services);
        let launch = Launch {
            file: seed.opening.file.clone(),
            sequence: seed.opening.sequence.clone(),
            look: seed.factory.appearances.current(),
            presentation: seed.presentation,
        };
        Wiring {
            edge,
            launch,
            requests: Rc::new(RefCell::new(Some(receive))),
            again,
        }
    }
}

/// Where the window's file was left: the store's memory, except that a place the launcher's pane
/// handed over is the first answer for the file it was handed with.
fn resume_of(seed: &Seed) -> Arc<dyn ResumeSource> {
    let stored = Arc::clone(&seed.factory.resume);
    match &seed.opening.resume {
        Resume::Nothing => stored,
        place @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }) => Arc::new(HandedResume::new(
            seed.opening.file.clone(),
            place.clone(),
            stored,
        )),
    }
}

/// How the app closes a window of its own doing: through the window's handle, which the close
/// request (the frame's button, the compositor) never puts to the viewer's veto, and so does not
/// ask again about changes the person has just answered for. Where there is no handle (a harness)
/// it is the host's close.
#[derive(Clone)]
pub(super) struct Closer {
    handle: Option<WindowHandle>,
    host: Option<WindowHost>,
}

impl Closer {
    /// The closer of the window the calling component renders in.
    pub(super) fn of_this_window(host: Option<WindowHost>) -> Self {
        Closer {
            handle: use_window_handle(),
            host,
        }
    }

    /// Close the window.
    pub(super) fn close(&self) {
        if let Some(handle) = &self.handle {
            handle.close();
        } else if let Some(host) = &self.host {
            host.host().close();
        }
    }
}

/// One viewer window with its host: the requests it makes are routed and carried out.
#[component]
fn Window(seed: Seed) -> Element {
    let wiring = use_hook(|| Wiring::new(&seed));
    let (edge, launch) = (wiring.edge.clone(), wiring.launch.clone());
    use_context_provider(|| edge);
    use_context_provider(|| launch);
    let looks = seed.factory.appearances.feed();
    use_context_provider(|| looks);
    let window = use_hook(try_consume_context::<WindowHost>);
    let closer = Closer::of_this_window(window.clone());
    let app = ds_blitz::use_app_handle();
    let sizer = use_window_sizer();
    let fit = use_hook(|| fit_of(seed.presentation, sizer, seed.factory.window_screen));
    let shown = use_hook(|| Rc::new(RefCell::new(Shown::default())));
    let hosting = Arc::clone(&seed.factory.hosting);
    let watching = use_hook(|| Rc::new(watch_for(&seed, &wiring.edge)));
    // Held for the window's life: a tool that appears is told to it until it closes.
    let _listening = use_hook(|| Rc::new(listen_for(&seed, &wiring.edge)));
    use_future(move || {
        let taken = wiring.requests.borrow_mut().take();
        let (window, shown, hosting) = (window.clone(), Rc::clone(&shown), Arc::clone(&hosting));
        let closer = closer.clone();
        let watching = Rc::clone(&watching);
        let (seed, app) = (seed.clone(), app.clone());
        let fit = Rc::clone(&fit);
        let (edge, again) = (wiring.edge.clone(), wiring.again.clone());
        async move {
            let Some(mut requests) = taken else { return };
            while let Some(request) = requests.next().await {
                let saves_text = matches!(request, HostRequest::SaveText(_));
                let (next, carry) = route(shown.take(), request);
                *shown.borrow_mut() = next;
                match carry {
                    Carry::Window(WindowTask::Close) => closer.close(),
                    Carry::Window(WindowTask::CopyText(text)) => {
                        match clipboard::write_text(&text) {
                            Ok(()) => edge.notify(Notice::say("Path copied")),
                            Err(error) => tell_problem(
                                &edge,
                                &format!("cannot copy: {error}"),
                                Some(Notice::say("Couldn\u{2019}t copy to the clipboard")),
                            ),
                        }
                    }
                    Carry::Window(WindowTask::OpenFiles(files)) => {
                        // Only the welcome window asks; a viewer window opens them all the same.
                        open_each(&seed.factory, app.as_ref(), files);
                    }
                    Carry::Window(WindowTask::Watch(file)) => {
                        if let Some(watching) = watching.as_ref() {
                            follow(watching, &file, &edge);
                        }
                    }
                    Carry::Window(WindowTask::Unwatch) => {
                        if let Some(watching) = watching.as_ref() {
                            watching.unwatch();
                        }
                    }
                    Carry::Window(WindowTask::Size { basis, file }) => {
                        let (fit, shown, window) =
                            (Rc::clone(&fit), Rc::clone(&shown), window.clone());
                        // A header is read off the UI thread; if the person has moved on by the
                        // time it is read, the next file has its own size to ask for.
                        spawn(async move {
                            let natural = natural_for(basis, file.clone()).await;
                            let now = shown
                                .borrow()
                                .file()
                                .map(|probed| probed.source.path().clone());
                            if now != file {
                                return;
                            }
                            if let Some(fit) = fit.as_ref() {
                                let state =
                                    window.as_ref().map(WindowHost::state).unwrap_or_default();
                                let tiled =
                                    window.as_ref().map(WindowHost::tiled).unwrap_or_default();
                                fit.loaded(natural, state, tiled);
                            }
                        });
                    }
                    Carry::Window(WindowTask::Reopen(presentation)) => {
                        reopen(
                            &seed,
                            shown.borrow().file(),
                            app.as_ref(),
                            presentation,
                            &edge,
                        );
                        closer.close();
                    }
                    Carry::Desktop(task) => {
                        let (doing, subject) = (Doing::of(&task), subject_of(&task));
                        let (done, shown) = (hosting.carry_out(task), Rc::clone(&shown));
                        let (watching, closer) = (Rc::clone(&watching), closer.clone());
                        let (edge, again) = (edge.clone(), again.clone());
                        spawn(async move {
                            let note = TaskNote { doing, subject };
                            ended(
                                done.await,
                                &shown,
                                &watching,
                                &closer,
                                (&edge, &again),
                                &note,
                            );
                        });
                    }
                    Carry::Declined(why) => {
                        // A save of the edited text the host will not make is an end the editor
                        // waits to hear, unless it is only waiting its turn.
                        let file = shown
                            .borrow()
                            .file()
                            .map(|probed| probed.source.path().clone());
                        if saves_text
                            && why != Declined::Queued
                            && let Some(file) = file
                        {
                            edge.saved(file, SaveEnd::Refused);
                        }
                        tell_declined(&edge, why);
                    }
                }
            }
        }
    });
    rsx! { ViewerApp {} }
}

/// The window's resize after load, through quire's sizer. The small window is never resized.
fn fit_of(
    presentation: Presentation,
    sizer: Option<WindowSizer>,
    replaced: Option<Extent>,
) -> Rc<Option<WindowFit>> {
    let sizer = match presentation {
        Presentation::Mini => None,
        Presentation::Window | Presentation::Peek | Presentation::Background => sizer,
    };
    Rc::new(sizer.map(|sizer| WindowFit::new(sizer, replaced)))
}

/// The seed of the window made again for `file` in `presentation`.
pub(super) fn remade(seed: &Seed, file: &FilePath, presentation: Presentation) -> Seed {
    Seed {
        factory: seed.factory.clone(),
        opening: Opening::around(file.clone()),
        presentation,
    }
}

/// What asking the desktop to keep the window just opened above the others comes to, for a
/// presentation that asks (the small window): `None` for any other.
pub(super) fn stacking_for(
    presentation: Presentation,
    desktop: &dyn StackingAsk,
) -> Option<StackingOutcome> {
    match presentation {
        Presentation::Mini => Some(desktop.ask(Stacking::KeepAbove)),
        Presentation::Window | Presentation::Peek | Presentation::Background => None,
    }
}

/// Open the file this window shows in a window of its own, in `presentation`, picking up where
/// the person is: the place the window last said is written first, so the new window's probe
/// finds it. The new window is opened through the app's handle, which only a real app has.
fn reopen(
    seed: &Seed,
    shown: Option<&anyview_ui::Opened>,
    app: Option<&AppHandle>,
    presentation: Presentation,
    edge: &Edge,
) {
    let (Some(probed), Some(app)) = (shown, app) else {
        return;
    };
    seed.factory.hosting.flush();
    let reopened = remade(seed, probed.source.path(), presentation);
    if open_in_window(app, reopened).is_err() {
        return;
    }
    if let Some(StackingOutcome::Unsupported) =
        stacking_for(presentation, seed.factory.stacking.as_ref())
    {
        tell_problem(
            edge,
            "the desktop does not let a window keep itself above the others",
            None,
        );
    }
}

/// What a task was doing and to which file, kept to word how it ended.
struct TaskNote {
    doing: Option<Doing>,
    subject: Option<FilePath>,
}

/// A task ended: say so, and follow the file if it was moved.
fn ended(
    outcome: Result<Outcome, tokio::task::JoinError>,
    shown: &Rc<RefCell<Shown>>,
    watching: &Rc<Option<WindowWatch>>,
    closer: &Closer,
    (edge, again): (&Edge, &UnboundedSender<HostRequest>),
    note: &TaskNote,
) {
    let outcome =
        outcome.unwrap_or_else(|error| Outcome::Failed(format!("a task was lost: {error}")));
    let after = shown.take().after(&outcome);
    *shown.borrow_mut() = after;
    // What waited for this save is asked again, now that the file is free.
    if matches!(outcome, Outcome::Written { .. } | Outcome::NotWritten(_)) {
        let (after, queued) = shown.take().next_queued();
        *shown.borrow_mut() = after;
        if let Some(request) = queued {
            let _gone = again.unbounded_send(request);
        }
    }
    // The file on disk is not the one the window opened: it looks at its stamp and reloads. The
    // text being edited is told instead, and keeps its place.
    if let Outcome::Written { file, kept: _ } = &outcome {
        if note.doing == Some(Doing::SaveText) {
            edge.saved(file.clone(), SaveEnd::Written);
        } else {
            edge.changed(file.clone());
        }
    }
    if note.doing == Some(Doing::SaveText)
        && matches!(outcome, Outcome::NotWritten(_))
        && let Some(file) = &note.subject
    {
        edge.saved(file.clone(), SaveEnd::Refused);
    }
    if let Outcome::Moved(path) = &outcome {
        edge.moved(path.clone());
        let now = shown.take().moved_to(path.clone());
        *shown.borrow_mut() = now;
        if let Some(watching) = watching.as_ref() {
            follow(watching, path, edge);
        }
    }
    // A chooser that ended without a file (cancelled, none on this desktop, failed) is told too, so
    // the window knows it may ask for another.
    if note.doing == Some(Doing::Pick) {
        let files = if let Outcome::Picked(files) = &outcome {
            files.clone()
        } else {
            Vec::new()
        };
        edge.chosen(files);
    }
    if let Outcome::Helped(helper, end) = &outcome {
        edge.helped(*helper, end.clone());
    }
    // The file plays with no window now: this window's part is done.
    if outcome == Outcome::Handed {
        closer.close();
    }
    tell(edge, note.doing, note.subject.as_ref(), &outcome);
}

/// The window's end of the program's file watcher, when there is a watcher: a change of the file
/// it watches is told to the window through its edge.
fn watch_for(seed: &Seed, edge: &Edge) -> Option<WindowWatch> {
    let watcher = seed.factory.watcher.as_ref()?;
    let edge = edge.clone();
    Some(watcher.window(move |file| edge.changed(file)))
}

/// This window's end of the tools the plugins run: told when one appears, whoever installed it.
fn listen_for(seed: &Seed, edge: &Edge) -> Option<Listening> {
    let helpers = seed.factory.helpers.as_ref()?;
    let edge = edge.clone();
    Some(helpers.listen(move |helper| edge.available(helper)))
}

/// Watch `file` for this window, and say so if the system refuses.
fn follow(watching: &WindowWatch, file: &FilePath, edge: &Edge) {
    if let Err(error) = watching.watch(file) {
        tell_problem(
            edge,
            &format!("cannot watch the file for changes: {error}"),
            None,
        );
    }
}
