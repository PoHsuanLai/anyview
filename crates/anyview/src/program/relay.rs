//! What the viewer's name receives: each file of a request, the first launch's or a forwarded one,
//! becomes an [`Opening`], and each opening a window of its own on the event loop.

use crate::window::{Factory, Opening, Seed, open_in_window};
use anyview_core::FilePath;
use anyview_platform::{Primary, Request};
use ds_blitz::AppHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;

/// The files a request asks to see, each for a window of its own. A peek and a play open the file
/// like any other until the quick-look window and the player exist.
pub fn files_of(request: Request) -> Vec<FilePath> {
    match request {
        Request::Open(files) => files,
        Request::Peek(file) | Request::Play(file) => vec![file],
    }
}

/// Make an opening of each file (listing its folder on the blocking pool) and hand it to
/// `openings`, until the windows are gone.
pub async fn open_each(files: Vec<FilePath>, openings: &UnboundedSender<Opening>) {
    for file in files {
        let Ok(opening) = tokio::task::spawn_blocking(move || Opening::around(file)).await else {
            continue;
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
        open_each(files_of(request), &openings).await;
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
