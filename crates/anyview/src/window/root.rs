//! The root components. Every window is a [`Window`]; the first one also listens for files to
//! open in windows of their own, which ds-blitz can only open from inside a component.

use super::opening::Opening;
use super::seed::Seed;
use crate::host::{Carry, Outcome, Shown, WindowTask, report, report_declined, route};
use anyview_core::FilePath;
use anyview_ui::{Edge, HostRequest, Launch, ViewerApp};
use dioxus::prelude::*;
use ds::prelude::WindowHost;
use ds_blitz::{WindowSpec, clipboard, open_window_with};
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

/// Where a new window is 1000 by 700 logical pixels until the viewer sizes windows to content.
const WINDOW: (u32, u32) = (1000, 700);

/// The files to open in more windows, given to the first window's root as a context. Cloneable
/// and sendable; the first root takes the receiver once.
#[derive(Debug, Clone)]
pub struct Inbox(Arc<Mutex<Option<UnboundedReceiver<Opening>>>>);

impl Inbox {
    /// An inbox over `receiver`.
    pub fn new(receiver: UnboundedReceiver<Opening>) -> Inbox {
        Inbox(Arc::new(Mutex::new(Some(receiver))))
    }

    fn take(&self) -> Option<UnboundedReceiver<Opening>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

/// The root of the first window: given the [`Seed`] and the [`Inbox`] as contexts
/// (`ds_blitz::AppConfig::with_context`).
pub fn first_root() -> Element {
    let seed = use_hook(consume_context::<Seed>);
    let inbox = use_hook(consume_context::<Inbox>);
    let shared = seed.factory.clone();
    use_future(move || {
        let taken = inbox.take();
        let shared = shared.clone();
        async move {
            let Some(mut openings) = taken else { return };
            while let Some(opening) = openings.next().await {
                open_another(Seed {
                    factory: shared.clone(),
                    opening,
                });
            }
        }
    });
    rsx! { Window { seed } }
}

/// The root of a window opened later.
fn later_root(seed: Seed) -> Element {
    rsx! { Window { seed } }
}

fn open_another(seed: Seed) {
    let title = title_of(&seed.opening.file);
    let spec = WindowSpec::new(title, WINDOW.0, WINDOW.1);
    if let Err(error) = open_window_with(spec, later_root, seed) {
        eprintln!("anyview: cannot open another window: {error}");
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
