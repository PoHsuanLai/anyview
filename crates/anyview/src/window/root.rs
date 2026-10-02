//! The root components. Every window is a [`Window`]: the program opens each through ds-blitz's
//! `AppHandle` with its [`Seed`] as props ([`window_root`]), and a harness test gives the
//! [`Seed`] as a context ([`seeded_root`]).

use super::seed::Seed;
use crate::host::{Carry, Outcome, Shown, WindowTask, report, report_declined, route};
use anyview_core::FilePath;
use anyview_ui::{Edge, HostRequest, Launch, ViewerApp};
use dioxus::prelude::*;
use ds::prelude::WindowHost;
use ds_blitz::{AppEnded, AppHandle, WindowSpec, clipboard};
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Where a new window is 1000 by 700 logical pixels until the viewer sizes windows to content.
const WINDOW: (u32, u32) = (1000, 700);

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
    let spec = WindowSpec::new(title_of(&seed.opening.file), WINDOW.0, WINDOW.1);
    app.open_window_with(spec, window_root, seed)
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
        });
        let launch = Launch {
            file: seed.opening.file.clone(),
            sequence: seed.opening.sequence.clone(),
            appearance: seed.factory.appearance,
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
    let shown = use_hook(|| Rc::new(RefCell::new(Shown::default())));
    let hosting = Arc::clone(&seed.factory.hosting);
    use_future(move || {
        let taken = wiring.requests.borrow_mut().take();
        let (window, shown, hosting) = (window.clone(), Rc::clone(&shown), Arc::clone(&hosting));
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
                    Carry::Desktop(task) => {
                        let (done, shown) = (hosting.carry_out(task), Rc::clone(&shown));
                        spawn(async move { ended(done.await, &shown) });
                    }
                    Carry::Declined(why) => report_declined(why),
                }
            }
        }
    });
    rsx! { ViewerApp {} }
}

/// A task ended: say so if it went wrong, and follow the file if it was moved.
fn ended(outcome: Result<Outcome, tokio::task::JoinError>, shown: &Rc<RefCell<Shown>>) {
    let outcome =
        outcome.unwrap_or_else(|error| Outcome::Failed(format!("a task was lost: {error}")));
    if let Outcome::Moved(path) = &outcome {
        let now = shown.take().moved_to(path.clone());
        *shown.borrow_mut() = now;
    }
    report(&outcome);
}
