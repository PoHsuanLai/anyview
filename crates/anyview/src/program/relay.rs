//! What the viewer's name receives: each file of a request, the first launch's or a forwarded one,
//! becomes an [`Arrival`]: an [`Opening`] for a window of its own on the event loop, or a file to
//! play with no window.

use crate::media::MediaHub;
use crate::window::{Factory, Opening, Seed, open_in_window};
use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, ModTime, Resume, SniffStep, Sniffed, Source, sniff,
};
use anyview_media::MediaError;
use anyview_platform::{Handoff, Primary, Request};
use anyview_ui::Presentation;
use ds_blitz::AppHandle;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;
use std::io::Read;

/// What one file of a request becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Arrival {
    /// A window of its own, with the files around it for the arrow keys.
    Window(Opening),
    /// A recording to play with no window.
    Background(FilePath),
}

/// What a request asks of one file, before the folder around it is listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// Show it in a window, with the files of its folder around it.
    Show(FilePath),
    /// Play it with no window.
    Play(FilePath),
    /// Show a file the launcher handed over with its results and its place.
    Handed(Handoff),
}

/// The files a request asks for and what is wanted of each. A peek shows like an open until the
/// quick-look window exists.
pub fn wants_of(request: Request) -> Vec<Want> {
    match request {
        Request::Open(files) => files.into_iter().map(Want::Show).collect(),
        Request::Peek(file) => vec![Want::Show(file)],
        Request::Play(file) => vec![Want::Play(file)],
        Request::Handoff(handed) => vec![Want::Handed(handed)],
    }
}

/// Make an arrival of each want (listing a window's folder on the blocking pool) and hand it to
/// `arrivals`, until the windows are gone.
pub async fn open_each(wants: Vec<Want>, arrivals: &UnboundedSender<Arrival>) {
    for want in wants {
        let arrival = match want {
            Want::Show(file) => {
                let Ok(opening) = tokio::task::spawn_blocking(move || Opening::around(file)).await
                else {
                    continue;
                };
                Arrival::Window(opening)
            }
            Want::Play(file) => Arrival::Background(file),
            Want::Handed(handed) => Arrival::Window(Opening::handed(handed)),
        };
        if arrivals.unbounded_send(arrival).is_err() {
            return;
        }
    }
}

/// Serve `primary` for the life of the process: every request another launch forwards opens its
/// files in new windows.
pub async fn relay(mut primary: Primary, arrivals: UnboundedSender<Arrival>) {
    while let Some(request) = primary.next().await {
        open_each(wants_of(request), &arrivals).await;
        if arrivals.is_closed() {
            return;
        }
    }
}

/// Carry out each arrival, until the channel closes or the app has ended: a window for each
/// opening (windows are independent, nothing here depends on an earlier one still being open),
/// and a player with no window for each recording to play in the background.
pub async fn open_windows(
    mut arrivals: UnboundedReceiver<Arrival>,
    app: AppHandle,
    factory: Factory,
    hub: MediaHub,
) {
    while let Some(arrival) = arrivals.next().await {
        match arrival {
            Arrival::Window(opening) => {
                let seed = Seed {
                    factory: factory.clone(),
                    opening,
                    presentation: Presentation::Window,
                };
                if open_in_window(&app, seed).is_err() {
                    return;
                }
            }
            Arrival::Background(file) => {
                let (hub, resume) = (hub.clone(), factory.resume.clone());
                let started = tokio::task::spawn_blocking(move || {
                    let stamp = stamp_of(&file);
                    let left = stamp.map_or(Resume::Nothing, |stamp| resume.recall(&file, stamp));
                    match (stamp, sniffed_of(&file)) {
                        (Some(stamp), Some(sniffed)) => {
                            hub.play_in_background(&Source::new(file, stamp), &sniffed, &left)
                        }
                        (None, _) | (_, None) => Err(MediaError::NotMedia),
                    }
                })
                .await;
                match started {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!("anyview: cannot play in the background: {error}"),
                    Err(error) => eprintln!("anyview: a task panicked: {error}"),
                }
            }
        }
    }
}

/// What the file is, from its first bytes: a background player needs its kind to find the program
/// that plays it.
fn sniffed_of(file: &FilePath) -> Option<Sniffed> {
    let mut head = Vec::new();
    std::fs::File::open(file.as_path())
        .and_then(|open| open.take(FileHead::MAX.0).read_to_end(&mut head))
        .ok()?;
    match sniff(&FileHead::new(&head), &file.file_name()?) {
        SniffStep::Done(sniffed) => Some(sniffed),
        SniffStep::LookInside(_) => None,
    }
}

/// The stamp the file has now, which is what a remembered place is checked against.
fn stamp_of(file: &FilePath) -> Option<FileStamp> {
    let meta = std::fs::metadata(file.as_path()).ok()?;
    Some(FileStamp {
        len: ByteLen(meta.len()),
        modified: meta
            .modified()
            .map_or(ModTime(0), ModTime::from_system_time),
    })
}
