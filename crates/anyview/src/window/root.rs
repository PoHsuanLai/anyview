//! The root components. Every window is a [`Window`]: the program opens each through ds-blitz's
//! `AppHandle` with its [`Seed`] as props ([`window_root`]), and a harness test gives the
//! [`Seed`] as a context ([`seeded_root`]).

use super::opening::Opening;
use super::seed::Seed;
use crate::host::{Carry, Outcome, Shown, WindowTask, WindowWatch, report, report_declined, route};
use anyview_core::FilePath;
use anyview_ui::{Edge, HostRequest, Launch, Presentation, ViewerApp};
use dioxus::prelude::*;
use ds::prelude::WindowHost;
use ds_blitz::{AppEnded, AppHandle, Decorations, WindowSpec, clipboard};
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Where a new window is 1000 by 700 logical pixels until the viewer sizes windows to content.
const WINDOW: (u32, u32) = (1000, 700);

/// The small window of a recording: 480 by 270, a sixteenth by nine picture.
const MINI: (u32, u32) = (480, 270);

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
    let spec = spec_for(&seed);
    app.open_window_with(spec, window_root, seed)
}

/// The window a seed asks for: a normal one, or the small borderless one whose capsule is its
/// only frame (the window draws nothing of its own on it).
fn spec_for(seed: &Seed) -> WindowSpec {
    let title = title_of(&seed.opening.file);
    match seed.presentation {
        Presentation::Mini => {
            WindowSpec::new(title, MINI.0, MINI.1).with_decorations(Decorations::Client)
        }
        Presentation::Window | Presentation::Peek | Presentation::Background => {
            WindowSpec::new(title, WINDOW.0, WINDOW.1)
        }
    }
}

fn title_of(file: &FilePath) -> String {
    file.file_name()
        .map_or_else(|| "anyview".to_owned(), |name| name.as_str().to_owned())
}

/// The window's end of its requests: the receiver its [`Edge`] sends to.
#[derive(Clone)]
struct Wiring {
    edge: Edge,
    launch: Launch,
    requests: Rc<RefCell<Option<UnboundedReceiver<HostRequest>>>>,
}

impl Wiring {
    fn new(seed: &Seed) -> Wiring {
        let (send, receive) = unbounded();
        let edge = Edge::new(Arc::clone(&seed.factory.workers), move |request| {
            // A window that closed has no receiver, and nobody is left to ask.
            let _gone = send.unbounded_send(request);
        })
        .with_resume_source(Arc::clone(&seed.factory.resume))
        .with_first_frames(Arc::clone(&seed.factory.first_frames))
        .with_media(Arc::clone(&seed.factory.media));
        let launch = Launch {
            file: seed.opening.file.clone(),
            sequence: seed.opening.sequence.clone(),
            appearance: seed.factory.appearance,
            presentation: seed.presentation,
        };
        Wiring {
            edge,
            launch,
            requests: Rc::new(RefCell::new(Some(receive))),
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
    let window = use_hook(try_consume_context::<WindowHost>);
    let app = ds_blitz::use_app_handle();
    let shown = use_hook(|| Rc::new(RefCell::new(Shown::default())));
    let hosting = Arc::clone(&seed.factory.hosting);
    let watching = use_hook(|| Rc::new(watch_for(&seed, &wiring.edge)));
    use_future(move || {
        let taken = wiring.requests.borrow_mut().take();
        let (window, shown, hosting) = (window.clone(), Rc::clone(&shown), Arc::clone(&hosting));
        let watching = Rc::clone(&watching);
        let (seed, app) = (seed.clone(), app.clone());
        async move {
            let Some(mut requests) = taken else { return };
            while let Some(request) = requests.next().await {
                let (next, carry) = route(shown.take(), request);
                *shown.borrow_mut() = next;
                match carry {
                    Carry::Window(WindowTask::Close) => {
                        if let Some(window) = &window {
                            window.host().close();
                        }
                    }
                    Carry::Window(WindowTask::CopyText(text)) => {
                        if let Err(error) = clipboard::write_text(&text) {
                            eprintln!("anyview: cannot copy: {error}");
                        }
                    }
                    Carry::Window(WindowTask::Watch(file)) => {
                        if let Some(watching) = watching.as_ref() {
                            follow(watching, &file);
                        }
                    }
                    Carry::Window(WindowTask::Unwatch) => {
                        if let Some(watching) = watching.as_ref() {
                            watching.unwatch();
                        }
                    }
                    Carry::Window(WindowTask::Reopen(presentation)) => {
                        reopen(&seed, shown.borrow().file(), app.as_ref(), presentation);
                        if let Some(window) = &window {
                            window.host().close();
                        }
                    }
                    Carry::Desktop(task) => {
                        let (done, shown) = (hosting.carry_out(task), Rc::clone(&shown));
                        let (watching, window) = (Rc::clone(&watching), window.clone());
                        spawn(async move { ended(done.await, &shown, &watching, window.as_ref()) });
                    }
                    Carry::Declined(why) => report_declined(why),
                }
            }
        }
    });
    rsx! { ViewerApp {} }
}

/// Open the file this window shows in a window of its own, in `presentation`, picking up where
/// the person is: the place the window last said is written first, so the new window's probe
/// finds it. The new window is opened through the app's handle, which only a real app has.
fn reopen(
    seed: &Seed,
    shown: Option<&anyview_ui::Probed>,
    app: Option<&AppHandle>,
    presentation: Presentation,
) {
    let (Some(probed), Some(app)) = (shown, app) else {
        return;
    };
    seed.factory.hosting.flush();
    let opening = Opening::around(probed.source.path().clone());
    let reopened = Seed {
        factory: seed.factory.clone(),
        opening,
        presentation,
    };
    if open_in_window(app, reopened).is_err() {
        return;
    }
    match presentation {
        Presentation::Mini => match seed
            .factory
            .stacking
            .ask(anyview_platform::Stacking::KeepAbove)
        {
            anyview_platform::StackingOutcome::Applied => {}
            anyview_platform::StackingOutcome::Unsupported => {
                eprintln!(
                    "anyview: the desktop does not let a window keep itself above the others"
                );
            }
        },
        Presentation::Window | Presentation::Peek | Presentation::Background => {}
    }
}

/// A task ended: say so if it went wrong, and follow the file if it was moved.
fn ended(
    outcome: Result<Outcome, tokio::task::JoinError>,
    shown: &Rc<RefCell<Shown>>,
    watching: &Rc<Option<WindowWatch>>,
    window: Option<&WindowHost>,
) {
    let outcome =
        outcome.unwrap_or_else(|error| Outcome::Failed(format!("a task was lost: {error}")));
    if let Outcome::Moved(path) = &outcome {
        let now = shown.take().moved_to(path.clone());
        *shown.borrow_mut() = now;
        if let Some(watching) = watching.as_ref() {
            follow(watching, path);
        }
    }
    // The file plays with no window now: this window's part is done.
    if outcome == Outcome::Handed
        && let Some(window) = window
    {
        window.host().close();
    }
    report(&outcome);
}

/// The window's end of the program's file watcher, when there is a watcher: a change of the file
/// it watches is told to the window through its edge.
fn watch_for(seed: &Seed, edge: &Edge) -> Option<WindowWatch> {
    let watcher = seed.factory.watcher.as_ref()?;
    let edge = edge.clone();
    Some(watcher.window(move |file| edge.changed(file)))
}

/// Watch `file` for this window, and say so if the system refuses.
fn follow(watching: &WindowWatch, file: &FilePath) {
    if let Err(error) = watching.watch(file) {
        eprintln!("anyview: cannot watch the file for changes: {error}");
    }
}
