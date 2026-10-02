//! What the viewer's name receives after the first window: each file of a forwarded request
//! becomes an [`Opening`] for the window layer, which opens a new window for it.

use crate::window::Opening;
use anyview_core::FilePath;
use anyview_platform::{Primary, Request};
use futures_channel::mpsc::UnboundedSender;

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
