//! The window of a launch with no file: its seed, the root that opens it, and the host's end of its
//! requests (the file dialog, and opening what is chosen or dropped).

use super::opening::Opening;
use super::root::open_in_window;
use super::seed::{Factory, Seed};
use crate::host::{Doing, Outcome, Task, tell};
use anyview_core::FilePath;
use anyview_ui::{Edge, HostRequest, Presentation, Services, WelcomeApp};
use dioxus::prelude::*;
use ds::prelude::WindowHost;
use ds_blitz::{AppEnded, AppHandle, WindowSize, WindowSpec};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use std::sync::Arc;

/// The welcome window is small: 520 by 380 logical pixels.
const WELCOME: WindowSize = WindowSize::new(520, 380);

/// What the welcome window is opened with: the shared wiring.
#[derive(Debug, Clone)]
pub struct WelcomeSeed {
    /// The shared wiring.
    pub factory: Factory,
}

impl PartialEq for WelcomeSeed {
    fn eq(&self, other: &WelcomeSeed) -> bool {
        Arc::ptr_eq(&self.factory.hosting, &other.factory.hosting)
            && Arc::ptr_eq(&self.factory.workers, &other.factory.workers)
    }
}

fn welcome_root(seed: WelcomeSeed) -> Element {
    rsx! { Welcome { seed } }
}

/// Open the welcome window, from any thread. It fails once the app has ended.
pub fn open_welcome(app: &AppHandle, factory: Factory) -> Result<(), AppEnded> {
    let spec = WindowSpec::new(anyview_core::APP_NAME.to_owned(), WELCOME);
    app.open_window_with(spec, welcome_root, WelcomeSeed { factory })
}

/// The root of the welcome window given its [`WelcomeSeed`] as a context, for a harness.
pub fn welcomed_root() -> Element {
    let seed = use_hook(consume_context::<WelcomeSeed>);
    rsx! { Welcome { seed } }
}

/// The welcome window with its host: it asks the file dialog and opens what it is given.
#[component]
fn Welcome(seed: WelcomeSeed) -> Element {
    let (send, receive) = unbounded();
    let edge = use_hook(|| {
        Edge::new(Services::new(
            Arc::clone(&seed.factory.workers),
            move |request| {
                // A window that closed has no receiver, and nobody is left to ask.
                let _gone = send.unbounded_send(request);
            },
        ))
    });
    let taken = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(Some(receive))));
    let provided = edge.clone();
    use_context_provider(|| provided);
    use_context_provider(|| seed.factory.appearances.feed());
    let window = use_hook(try_consume_context::<WindowHost>);
    let app = ds_blitz::use_app_handle();
    use_future(move || {
        let requests = taken.borrow_mut().take();
        let (edge, seed) = (edge.clone(), seed.clone());
        let (window, app) = (window.clone(), app.clone());
        async move {
            let Some(mut requests) = requests else { return };
            while let Some(request) = requests.next().await {
                if let HostRequest::PickFile = request {
                    let done = seed.factory.hosting.carry_out(Task::PickFile);
                    let edge = edge.clone();
                    spawn(async move { picked(done.await, &edge) });
                } else if let HostRequest::OpenFiles(files) = request {
                    if open_each(&seed.factory, app.as_ref(), files)
                        && let Some(window) = &window
                    {
                        window.host().close();
                    }
                } else if let HostRequest::CloseWindow = request
                    && let Some(window) = &window
                {
                    window.host().close();
                }
                // The welcome window has no file for any other request to be about.
            }
        }
    });
    rsx! { WelcomeApp {} }
}

/// The file dialog ended: the files it chose go back to the window, anything else is told.
fn picked(done: Result<Outcome, tokio::task::JoinError>, edge: &Edge) {
    let outcome = done.unwrap_or_else(|error| Outcome::Failed(format!("a task was lost: {error}")));
    if let Outcome::Picked(files) = &outcome {
        edge.chosen(files.clone());
    }
    tell(edge, Some(Doing::Pick), None, &outcome);
}

/// A window for each of `files`. True when at least one opened, so the welcome window can go.
pub(super) fn open_each(factory: &Factory, app: Option<&AppHandle>, files: Vec<FilePath>) -> bool {
    let Some(app) = app else {
        return false;
    };
    files
        .into_iter()
        .map(|file| Seed {
            factory: factory.clone(),
            opening: Opening::around(file),
            presentation: Presentation::Window,
        })
        .map(|seed| open_in_window(app, seed).is_ok())
        // Every window is opened: `any` alone would stop at the first.
        .collect::<Vec<bool>>()
        .contains(&true)
}
