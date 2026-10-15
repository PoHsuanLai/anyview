//! What the viewer's name receives: each file of a request, the first launch's or a forwarded one,
//! becomes an [`Arrival`]: an [`Opening`] for a window of its own on the event loop, or a file to
//! play with no window.

use crate::media::MediaHub;
use crate::window::{Factory, Opening, Seed, open_in_window, open_welcome};
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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long the thread that hears other launches waits for one before it looks whether the
/// program is ending.
const LISTEN_TICK: Duration = Duration::from_millis(250);

/// What one file of a request becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Arrival {
    /// A window of its own, with the files around it for the arrow keys.
    Window(Opening),
    /// A recording to play with no window.
    Background(FilePath),
    /// The welcome window: a launch that named no file.
    Welcome(WelcomeWhen),
}

/// When a welcome window is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeWhen {
    /// Another launch with no file asked for it: it opens.
    Always,
    /// The first launch had no file, and the bus may still deliver the request that started it:
    /// the window opens only if no other has.
    IfNoWindow,
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

/// Serve the requests `listen` hands over for the life of the process: every request another launch
/// forwards opens its files in new windows.
pub async fn relay(
    mut requests: tokio::sync::mpsc::UnboundedReceiver<Request>,
    arrivals: UnboundedSender<Arrival>,
) {
    while let Some(request) = requests.recv().await {
        let wants = wants_of(request);
        if wants.is_empty() {
            // A launch with no file: what a person sees is a window to choose one in.
            let _gone = arrivals.unbounded_send(Arrival::Welcome(WelcomeWhen::Always));
        }
        open_each(wants, &arrivals).await;
        if arrivals.is_closed() {
            return;
        }
    }
}

/// The door the viewer keeps open for other launches: the thread that hears them, which holds the
/// [`Primary`] until the program is ending.
#[derive(Debug)]
pub struct Doorway {
    stopping: Arc<AtomicBool>,
    listening: Option<JoinHandle<Vec<Request>>>,
}

impl Doorway {
    /// Close the door, then return what had come through it and was not yet taken up. A launch that
    /// knocks after this is told the viewer is closing and runs on its own, so nothing a launch was
    /// told "taken" is lost.
    pub fn close(mut self) -> Vec<Request> {
        self.stopping.store(true, Ordering::SeqCst);
        self.listening
            .take()
            .and_then(|listening| listening.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for Doorway {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
}

/// Listen at `primary` on a thread of its own, because it waits: the requests it hears come out of
/// the receiver, and the [`Doorway`] closes the door when the program ends.
pub fn listen(primary: Primary) -> (tokio::sync::mpsc::UnboundedReceiver<Request>, Doorway) {
    let (sender, requests) = tokio::sync::mpsc::unbounded_channel();
    let stopping = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&stopping);
    let listening = std::thread::Builder::new()
        .name("anyview-requests".to_owned())
        .spawn(move || {
            let mut primary = primary;
            let mut unsent = Vec::new();
            while !stop.load(Ordering::SeqCst) {
                let waited = Instant::now();
                match primary.next_within(LISTEN_TICK) {
                    Some(request) => {
                        if let Err(refused) = sender.send(request) {
                            unsent.push(refused.0);
                            break;
                        }
                    }
                    // Nothing came, or nothing can: do not spin on a door that is shut.
                    None => std::thread::sleep(LISTEN_TICK.saturating_sub(waited.elapsed())),
                }
            }
            unsent.extend(primary.close());
            unsent
        })
        .inspect_err(|error| log::error!("cannot listen for other launches: {error}"))
        .ok();
    (
        requests,
        Doorway {
            stopping,
            listening,
        },
    )
}

/// Start another viewer for `requests` that reached this one as it was ending, so a person who
/// double-clicked a file still gets a window. A handoff's results and place are not carried: its
/// file opens like any other.
pub fn hand_on(requests: Vec<Request>) {
    if requests.is_empty() {
        return;
    }
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => {
            log::error!(
                "cannot start another viewer for {} requests: {error}",
                requests.len()
            );
            return;
        }
    };
    for request in requests {
        let mut command = std::process::Command::new(&program);
        match &request {
            Request::Open(files) => command.arg("--").args(files.iter().map(FilePath::as_path)),
            Request::Peek(file) => command.args(["--peek", "--"]).arg(file.as_path()),
            Request::Play(file) => command.args(["--play", "--"]).arg(file.as_path()),
            Request::Handoff(handed) => command.arg("--").arg(handed.file.as_path()),
        };
        command.stdin(std::process::Stdio::null());
        if let Err(error) = command.spawn() {
            log::error!("cannot start another viewer for {request:?}: {error}");
        }
    }
}

/// The app has ended with these arrivals unopened: start another viewer for them and for what is
/// still queued, so a launch that was told "taken" still gets its window.
fn hand_over(mut requests: Vec<Request>, arrivals: &mut UnboundedReceiver<Arrival>) {
    arrivals.close();
    while let Ok(arrival) = arrivals.try_recv() {
        match arrival {
            Arrival::Window(opening) => requests.push(Request::Open(vec![opening.file])),
            Arrival::Background(file) => requests.push(Request::Play(file)),
            Arrival::Welcome(WelcomeWhen::Always) => requests.push(Request::Open(Vec::new())),
            Arrival::Welcome(WelcomeWhen::IfNoWindow) => {}
        }
    }
    if requests.is_empty() {
        return;
    }
    log::warn!("the viewer ended with {} requests unopened", requests.len());
    hand_on(requests);
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
    let mut opened = 0_u32;
    while let Some(arrival) = arrivals.next().await {
        match arrival {
            Arrival::Welcome(when) => {
                let wanted = match when {
                    WelcomeWhen::Always => true,
                    WelcomeWhen::IfNoWindow => opened == 0,
                };
                if wanted && open_welcome(&app, factory.clone()).is_err() {
                    let again = match when {
                        WelcomeWhen::Always => vec![Request::Open(Vec::new())],
                        WelcomeWhen::IfNoWindow => Vec::new(),
                    };
                    hand_over(again, &mut arrivals);
                    return;
                }
                opened += 1;
            }
            Arrival::Window(opening) => {
                let file = opening.file.clone();
                let seed = Seed {
                    factory: factory.clone(),
                    opening,
                    presentation: Presentation::Window,
                };
                if open_in_window(&app, seed).is_err() {
                    hand_over(vec![Request::Open(vec![file])], &mut arrivals);
                    return;
                }
                opened += 1;
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
                    Ok(Err(error)) => log::warn!("cannot play in the background: {error}"),
                    Err(error) => log::error!("a task panicked: {error}"),
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
