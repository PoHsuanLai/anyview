//! What the viewer's name receives: each file of a request, the first launch's or a forwarded one,
//! becomes an [`Opening`], and each opening a window of its own on the event loop.

use crate::window::{Factory, Opening, Seed, open_in_window};
use anyview_core::FilePath;
use anyview_platform::{Handoff, Primary, Request};
use ds_blitz::AppHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;

/// What a request asks to see, each for a window of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
    /// A file, with the files of its folder around it.
    Around(FilePath),
    /// A file the launcher handed over with its results and its place.
    Handed(Handoff),
}

/// The windows a request asks for. A peek and a play open the file like any other until the
/// quick-look window and the player exist.
pub fn wanted_by(request: Request) -> Vec<Wanted> {
    match request {
        Request::Open(files) => files.into_iter().map(Wanted::Around).collect(),
        Request::Peek(file) | Request::Play(file) => vec![Wanted::Around(file)],
        Request::Handoff(handed) => vec![Wanted::Handed(handed)],
    }
}

/// Make an opening of each (listing a folder on the blocking pool) and hand it to `openings`,
/// until the windows are gone.
pub async fn open_each(wanted: Vec<Wanted>, openings: &UnboundedSender<Opening>) {
    for one in wanted {
        let opening = match one {
            Wanted::Around(file) => {
                match tokio::task::spawn_blocking(move || Opening::around(file)).await {
                    Ok(opening) => opening,
                    Err(_) => continue,
                }
            }
            Wanted::Handed(handed) => Opening::handed(handed),
        };
        if openings.unbounded_send(opening).is_err() {
            return;
        }
    }
}

/// Serve `primary` for the life of the process: every request another launch forwards opens its
/// files in new windows.
pub async fn relay(mut primary: Primary, openings: UnboundedSender<Opening>) {
    while let Some(request) = primary.next().await {
        open_each(wanted_by(request), &openings).await;
        if openings.is_closed() {
            return;
        }
    }
}

/// Open a window for each opening, until the channel closes or the app has ended. Windows are
/// independent: nothing here depends on an earlier one still being open.
pub async fn open_windows(
    mut openings: UnboundedReceiver<Opening>,
    app: AppHandle,
    factory: Factory,
) {
    while let Some(opening) = openings.next().await {
        let seed = Seed {
            factory: factory.clone(),
            opening,
        };
        if open_in_window(&app, seed).is_err() {
            return;
        }
    }
}
