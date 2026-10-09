//! The program's players: which sessions are running, the desktop's one now-playing entry and the
//! controls that come back from it, and the sessions that play with no window.
//!
//! A session is a window's (its line is held by the window, and the hub only watches it) or a
//! background one (the hub holds it, and holds the event loop alive while it plays). The desktop
//! has one entry, so it shows the session that last played; its controls are carried out on
//! that one.

use super::engine::{BuiltinAbility, Chosen, Engine, builtin_ability, choose};
use super::line::LiveLine;
use super::orders::{Home, Order, orders_for};
use super::plugins::{MediaPlugins, PlayRoute};
use crate::runtime::{Actor, Mailbox, RuntimeError, UiWaker};
use anyview_core::{FilePath, Resume, Sniffed, Source};
use anyview_media::{AudioDriver, MediaCommand, MediaError, PictureSlot, ShotContent};
use anyview_platform::{MediaControl, MediaSession, MediaState, PlaybackStatus};
use anyview_plugin::Subject;
use anyview_ui::MediaNotice;
use ds_blitz::{AppHandle, AppHold};
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

/// How long a saved frame is waited for: mpv writes it before the call returns, so this is the
/// time to hear of it.
const SHOT_WAIT: Duration = Duration::from_secs(10);

/// Names one running player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SessionId(pub(super) u32);

/// What the sessions tell the task that serves the desktop's entry.
#[derive(Debug)]
pub(super) enum Update {
    /// A session's entry changed.
    State(SessionId, Box<MediaState>),
    /// A session is gone.
    Gone(SessionId),
}

/// Who holds a session's line.
enum Owner {
    /// A window holds it; the hub watches.
    Window(Weak<LiveLine>),
    /// The hub holds it, and the event loop with it.
    Background(Background),
}

/// A session with no window: its line, and the claim that keeps the event loop running while it
/// plays. Dropping it ends the player and lets the loop go.
struct Background {
    line: Arc<LiveLine>,
    _hold: Option<AppHold>,
}

struct Entry {
    id: SessionId,
    file: FilePath,
    owner: Owner,
}

impl Entry {
    fn home(&self) -> Home {
        match self.owner {
            Owner::Window(_) => Home::Window,
            Owner::Background(_) => Home::Background,
        }
    }

    /// Tell the window of a window's session; a background session has none.
    fn tell(&self, notice: MediaNotice) {
        if let Owner::Window(line) = &self.owner
            && let Some(line) = line.upgrade()
        {
            line.tell(notice);
        }
    }

    fn send(&self, command: MediaCommand) {
        match &self.owner {
            Owner::Window(line) => {
                if let Some(line) = line.upgrade() {
                    line.command(command);
                }
            }
            Owner::Background(background) => background.line.command(command),
        }
    }
}

/// Why a frame was not saved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ShotError {
    /// No window is playing the file.
    #[error("no window is playing the file")]
    NoWindow,
    /// The player ended before it saved the frame.
    #[error("the player ended before it saved the frame")]
    PlayerEnded,
    /// The player said nothing within the wait.
    #[error("the player did not save the frame in time")]
    TimedOut,
    /// The player could not save the frame, for the reason it gave.
    #[error("{0}")]
    Player(String),
}

type ShotWaiter = (SessionId, FilePath, oneshot::Sender<Result<(), ShotError>>);

/// The shared state of the hub.
pub(super) struct Inner {
    next: AtomicU32,
    updates: UnboundedSender<Update>,
    sessions: Mutex<Vec<Entry>>,
    shots: Mutex<Vec<ShotWaiter>>,
    app: Option<AppHandle>,
    audio: AudioDriver,
    plugins: Arc<MediaPlugins>,
    runtime: tokio::runtime::Handle,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    pub(super) fn next_id(&self) -> SessionId {
        SessionId(self.next.fetch_add(1, Ordering::Relaxed))
    }

    pub(super) fn audio(&self) -> AudioDriver {
        self.audio
    }

    pub(super) fn plugins(&self) -> &MediaPlugins {
        &self.plugins
    }

    /// The player for `file`, a recording of `sniffed`'s kind, or why there is none.
    pub(super) fn player(&self, file: &FilePath, sniffed: &Sniffed) -> Result<Chosen, MediaError> {
        let subject = Subject {
            kind: sniffed.kind(),
            mime: Some(sniffed.mime()),
        };
        let route = self.plugins.player(&subject);
        let ability = match &route {
            PlayRoute::Ready(_) => BuiltinAbility::CannotDecode,
            PlayRoute::Missing(_) | PlayRoute::Unserved => {
                builtin_ability(sniffed.kind(), file.as_path(), self.audio)
            }
        };
        match choose(route, ability) {
            Chosen::Missing(missing) => Err(MediaError::PlayerMissing(missing.package.name())),
            Chosen::Unserved => Err(MediaError::NotMedia),
            Chosen::NoSound => Err(MediaError::NoSoundOutput),
            chosen @ (Chosen::Mpv(_) | Chosen::Builtin) => Ok(chosen),
        }
    }

    /// A session's entry changed.
    pub(super) fn state(&self, id: SessionId, state: MediaState) {
        let _closed = self.updates.send(Update::State(id, Box::new(state)));
    }

    /// A window's session is gone, because its line was dropped.
    pub(super) fn gone(&self, id: SessionId) {
        locked(&self.sessions).retain(|entry| entry.id != id);
        let _closed = self.updates.send(Update::Gone(id));
    }

    /// A background session's recording ended: the session ends, and the loop is let go.
    pub(super) fn finished(&self, id: SessionId) {
        let owner = {
            let mut sessions = locked(&self.sessions);
            let at = sessions.iter().position(|entry| entry.id == id);
            at.map(|at| sessions.remove(at))
        };
        if let Some(entry) = owner {
            // The session's own thread cannot join itself: a blocking worker drops the line.
            self.runtime.spawn_blocking(move || drop(entry));
        }
        let _closed = self.updates.send(Update::Gone(id));
    }

    /// A saved frame's outcome, for the export waiting on it.
    pub(super) fn shot_done(&self, id: SessionId, to: &FilePath, result: Result<(), ShotError>) {
        let waiter = {
            let mut shots = locked(&self.shots);
            let at = shots
                .iter()
                .position(|(who, path, _)| *who == id && path == to);
            at.map(|at| shots.remove(at))
        };
        if let Some((_, _, tell)) = waiter {
            let _gone = tell.send(result);
        }
    }

    fn register(&self, id: SessionId, file: FilePath, owner: Owner) {
        locked(&self.sessions).push(Entry { id, file, owner });
    }

    /// Tell the window of session `id` something.
    fn tell(&self, id: SessionId, notice: MediaNotice) {
        if let Some(entry) = locked(&self.sessions).iter().find(|e| e.id == id) {
            entry.tell(notice);
        }
    }

    /// Carry out what a control asks of the session whose entry is `state`.
    fn control(&self, id: SessionId, control: MediaControl, state: &MediaState) {
        let home = locked(&self.sessions)
            .iter()
            .find(|entry| entry.id == id)
            .map(Entry::home);
        let Some(home) = home else {
            return;
        };
        for order in orders_for(control, state, home) {
            match order {
                Order::Send(command) => {
                    if let Some(entry) = locked(&self.sessions).iter().find(|e| e.id == id) {
                        entry.send(command);
                    }
                }
                Order::End => self.finished(id),
                Order::Next => self.tell(id, MediaNotice::Next),
                Order::Previous => self.tell(id, MediaNotice::Previous),
                Order::Quit => {
                    if let Some(app) = &self.app {
                        let _ended = app.quit();
                    }
                }
            }
        }
    }
}

/// The program's players. Cloning shares them.
#[derive(Clone)]
pub struct MediaHub {
    pub(super) inner: Arc<Inner>,
}

impl std::fmt::Debug for MediaHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaHub").finish_non_exhaustive()
    }
}

/// A waker that wakes nobody: the mailbox of a session no window watches.
struct Silent;

impl UiWaker for Silent {
    fn wake(&self) {}
}

impl MediaHub {
    /// A hub whose desktop entry is made by `register` when the first player speaks, so a viewer
    /// that plays nothing shows nothing to the desktop, and served on `runtime`. `app` is the event
    /// loop that background sessions hold open and that Quit ends; `audio` is the sound driver every
    /// session plays on; `plugins` are what plays, probes and exports a recording.
    pub fn start<S, F, Fut>(
        runtime: &tokio::runtime::Handle,
        register: F,
        app: Option<AppHandle>,
        audio: AudioDriver,
        plugins: Arc<MediaPlugins>,
    ) -> MediaHub
    where
        S: MediaSession + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = S> + Send + 'static,
    {
        let (updates, inbox) = unbounded_channel();
        let inner = Arc::new(Inner {
            next: AtomicU32::new(1),
            updates,
            sessions: Mutex::new(Vec::new()),
            shots: Mutex::new(Vec::new()),
            app,
            audio,
            plugins,
            runtime: runtime.clone(),
        });
        runtime.spawn(serve(register, inbox, Arc::downgrade(&inner)));
        MediaHub { inner }
    }

    /// Start a session playing `file` with no window, picking up where `resume` says, and hold
    /// the event loop open while it plays. Blocking: it makes a graphics device and a player.
    pub fn play_in_background(
        &self,
        source: &Source,
        sniffed: &Sniffed,
        resume: &Resume,
    ) -> Result<(), MediaError> {
        let file = source.path();
        let engine = match self.inner.player(file, sniffed)? {
            Chosen::Mpv(host) => {
                let (device, queue) = anyview_media::headless_device()?;
                Engine::Mpv {
                    device,
                    queue,
                    audio: self.inner.audio(),
                    host,
                    sink: Box::new(super::sink::NoSink),
                }
            }
            Chosen::Builtin => Engine::Builtin {
                audio: self.inner.audio(),
            },
            Chosen::NoSound | Chosen::Missing(_) | Chosen::Unserved => {
                return Err(MediaError::NotMedia);
            }
        };
        let reading = self.inner.plugins().reading(source, sniffed);
        let id = self.inner.next_id();
        let art = reading
            .cover
            .as_ref()
            .map(|cover| anyview_platform::Artwork {
                png: Arc::from(cover.png.as_slice()),
            });
        let snapshot = super::snapshot::Snapshot::new(
            file,
            &reading.tags,
            art,
            anyview_platform::Ability::Cannot,
            anyview_platform::TrackSerial(id.0),
        );
        let plan = super::actor::Plan {
            engine,
            file: file.clone(),
            snapshot,
            hub: Arc::downgrade(&self.inner),
            id,
            home: Home::Background,
        };
        let (mailbox, outbox) = Mailbox::new(Silent);
        let posting = outbox.clone();
        let actor = Actor::spawn("anyview-media", outbox, move |wake| {
            super::actor::MediaActor::start(plan, &wake)
        })
        .map_err(|error: RuntimeError| MediaError::Player(error.to_string()))?;
        for command in
            std::iter::once(MediaCommand::Slot(PictureSlot::Empty)).chain(restoring(resume))
        {
            let _ended = actor.send(command);
        }
        let line = Arc::new(LiveLine {
            actor,
            mailbox,
            outbox: posting,
            id,
            hub: Arc::downgrade(&self.inner),
        });
        let hold = self.inner.app.as_ref().and_then(|app| app.hold().ok());
        self.inner.register(
            id,
            file.clone(),
            Owner::Background(Background { line, _hold: hold }),
        );
        Ok(())
    }

    /// The plugins that play, read and write recordings.
    pub fn plugins(&self) -> &MediaPlugins {
        self.inner.plugins()
    }

    /// Whether a background session is playing: the program has no window to leave open for it.
    pub fn plays_in_background(&self) -> bool {
        locked(&self.inner.sessions)
            .iter()
            .any(|entry| entry.home() == Home::Background)
    }

    /// Register the window's session `line` for `file`, so the desktop's entry and the controls
    /// reach it.
    pub(super) fn register_window(&self, id: SessionId, file: FilePath, line: &Arc<LiveLine>) {
        self.inner
            .register(id, file, Owner::Window(Arc::downgrade(line)));
    }

    /// Save the frame the window playing `file` shows to `to` (the extension picks the format),
    /// at the picture's own resolution.
    pub async fn screenshot(
        &self,
        file: &FilePath,
        to: FilePath,
        content: ShotContent,
    ) -> Result<(), ShotError> {
        let (tell, heard) = oneshot::channel();
        {
            let sessions = locked(&self.inner.sessions);
            let Some(entry) = sessions
                .iter()
                .find(|entry| entry.home() == Home::Window && &entry.file == file)
            else {
                return Err(ShotError::NoWindow);
            };
            locked(&self.inner.shots).push((entry.id, to.clone(), tell));
            entry.send(MediaCommand::Screenshot { to, content });
        }
        match tokio::time::timeout(SHOT_WAIT, heard).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ShotError::PlayerEnded),
            Err(_) => Err(ShotError::TimedOut),
        }
    }

    /// End every background session.
    pub fn stop_background(&self) {
        let ids: Vec<SessionId> = locked(&self.inner.sessions)
            .iter()
            .filter(|entry| entry.home() == Home::Background)
            .map(|entry| entry.id)
            .collect();
        ids.into_iter().for_each(|id| self.inner.finished(id));
    }
}

/// The instructions that put a place left back into a player that has just opened.
pub(super) fn restoring(resume: &Resume) -> Vec<MediaCommand> {
    use anyview_core::StreamKind;
    match resume {
        Resume::Media {
            at,
            volume,
            audio,
            subtitles,
        } => {
            let seek = (at.0 > 0).then_some(MediaCommand::Seek(*at));
            seek.into_iter()
                .chain([
                    MediaCommand::SetVolume(*volume),
                    MediaCommand::SelectTrack {
                        kind: StreamKind::Audio,
                        choice: *audio,
                    },
                    MediaCommand::SelectTrack {
                        kind: StreamKind::Subtitles,
                        choice: *subtitles,
                    },
                ])
                .collect()
        }
        Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }
        | Resume::Nothing => Vec::new(),
    }
}

/// Which session the desktop's entry shows: the one that just started playing; otherwise the one
/// already shown, otherwise the one that just spoke.
pub(super) fn chosen(
    shown: Option<SessionId>,
    spoke: SessionId,
    state: &MediaState,
) -> Option<SessionId> {
    match state.status {
        PlaybackStatus::Playing => Some(spoke),
        PlaybackStatus::Paused | PlaybackStatus::Stopped => shown.or(Some(spoke)),
    }
}

/// What the entry of the shown session becomes after `update`, if it changes: the entry itself, or
/// the stopped one when the shown session is gone and none is left.
fn entry_after(
    update: Update,
    states: &mut HashMap<SessionId, MediaState>,
    shown: &mut Option<SessionId>,
) -> Option<MediaState> {
    match update {
        Update::State(id, state) => {
            *shown = chosen(*shown, id, &state);
            states.insert(id, *state);
            shown.and_then(|shown| states.get(&shown).cloned())
        }
        Update::Gone(id) => {
            states.remove(&id);
            if *shown == Some(id) {
                *shown = states.keys().next().copied();
                Some(
                    shown
                        .and_then(|shown| states.get(&shown).cloned())
                        .unwrap_or_else(MediaState::stopped),
                )
            } else {
                None
            }
        }
    }
}

/// The task that serves the desktop's one entry for the life of the program: it makes the entry
/// when the first player speaks, publishes the session that is shown, and carries the desktop's
/// controls out on it.
async fn serve<S, F, Fut>(register: F, mut updates: UnboundedReceiver<Update>, hub: Weak<Inner>)
where
    S: MediaSession,
    F: FnOnce() -> Fut,
    Fut: Future<Output = S>,
{
    let mut states: HashMap<SessionId, MediaState> = HashMap::new();
    let mut shown: Option<SessionId> = None;
    let Some(first) = updates.recv().await else {
        return;
    };
    let mut session = register().await;
    let mut pending = entry_after(first, &mut states, &mut shown);
    loop {
        if let Some(state) = pending.take()
            && let Err(error) = session.publish(&state).await
        {
            eprintln!("anyview: cannot publish the now-playing entry: {error}");
        }
        tokio::select! {
            control = session.next_control() => {
                let Some(control) = control else { return };
                let (Some(hub), Some(id)) = (hub.upgrade(), shown) else { continue };
                if let Some(state) = states.get(&id) {
                    hub.control(id, control, state);
                }
            }
            update = updates.recv() => {
                let Some(update) = update else { return };
                pending = entry_after(update, &mut states, &mut shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{MediaTime, Percent, StreamKind, TrackChoice, TrackId, Volume};

    fn state(status: PlaybackStatus) -> MediaState {
        MediaState {
            status,
            ..MediaState::stopped()
        }
    }

    #[test]
    fn the_entry_shows_the_session_that_just_played_else_the_one_it_has() {
        let (a, b) = (SessionId(1), SessionId(2));
        /// Name, the session shown, the one that spoke, its status, the session shown after.
        type Case = (&'static str, Option<u32>, u32, PlaybackStatus, Option<u32>);
        const CASES: &[Case] = &[
            (
                "the first to speak is shown",
                None,
                1,
                PlaybackStatus::Paused,
                Some(1),
            ),
            (
                "one that starts playing takes the entry",
                Some(1),
                2,
                PlaybackStatus::Playing,
                Some(2),
            ),
            (
                "one that pauses does not",
                Some(1),
                2,
                PlaybackStatus::Paused,
                Some(1),
            ),
            (
                "one that stopped does not",
                Some(1),
                2,
                PlaybackStatus::Stopped,
                Some(1),
            ),
            (
                "the shown one stays shown when it pauses",
                Some(2),
                2,
                PlaybackStatus::Paused,
                Some(2),
            ),
        ];
        for (name, shown, spoke, status, want) in CASES {
            let id = |n: u32| if n == 1 { a } else { b };
            assert_eq!(
                chosen(shown.map(id), id(*spoke), &state(*status)),
                want.map(id),
                "{name}"
            );
        }
    }

    #[test]
    fn a_place_left_is_put_back_as_instructions_and_other_places_say_nothing() {
        let media = Resume::Media {
            at: MediaTime::from_secs(30),
            volume: Volume::clamped(Percent(70)),
            audio: TrackChoice::Track(TrackId(2)),
            subtitles: TrackChoice::Off,
        };
        assert_eq!(
            restoring(&media),
            vec![
                MediaCommand::Seek(MediaTime::from_secs(30)),
                MediaCommand::SetVolume(Volume::clamped(Percent(70))),
                MediaCommand::SelectTrack {
                    kind: StreamKind::Audio,
                    choice: TrackChoice::Track(TrackId(2)),
                },
                MediaCommand::SelectTrack {
                    kind: StreamKind::Subtitles,
                    choice: TrackChoice::Off,
                },
            ]
        );
        let at_start = Resume::Media {
            at: MediaTime::default(),
            volume: Volume::FULL,
            audio: TrackChoice::Auto,
            subtitles: TrackChoice::Auto,
        };
        assert_eq!(restoring(&at_start).len(), 3, "no seek to the start");
        assert!(restoring(&Resume::Nothing).is_empty());
    }
}
