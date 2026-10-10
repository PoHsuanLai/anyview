//! A recording in a pane: it plays in the pane when the host gave the edge a player, and is the
//! host's to open elsewhere when it did not.

use crate::support::{Host, hosted_with, media_file};
use anyview_core::{Facts, FilePath, MediaTags};
use anyview_pane::{MediaHost, PaneRequest};
use anyview_ui::{
    MediaLine, MediaNotice, MediaOffer, MediaPlayback, MediaStart, MediaStarted, OpenError,
    PlayerCommand, SlotPixels,
};
use ds_harness::{Query, Viewport};
use std::path::Path;
use std::sync::Arc;

/// A player that starts and says nothing: enough for a pane to show the stage of a playing file.
#[derive(Debug)]
struct Silent;

impl MediaLine for Silent {
    fn send(&self, _command: PlayerCommand) {}

    fn resize(&self, _slot: Option<SlotPixels>) {}

    fn drain(&self) -> Vec<MediaNotice> {
        Vec::new()
    }
}

#[derive(Debug)]
struct Plays;

impl MediaHost for Plays {
    fn start(&self, _start: MediaStart) -> Result<MediaStarted, OpenError> {
        Ok(MediaStarted {
            playback: MediaPlayback::Line(Arc::new(Silent)),
            offer: MediaOffer::new(Vec::new(), None),
            tags: MediaTags::default(),
            facts: Facts::empty(),
            length: None,
            cover: None,
        })
    }
}

fn in_a_corner() -> Viewport {
    Viewport {
        width: 640,
        height: 400,
        scale_percent: 100,
    }
}

fn open_in_a_pane(file: &Path, media: Option<Arc<dyn MediaHost>>) -> Host {
    hosted_with(vec![(file.to_path_buf(), None)], in_a_corner(), None, media)
}

#[test]
fn a_recording_plays_in_the_pane_only_where_the_host_gave_it_a_player() {
    let dir = tempfile::tempdir().unwrap();
    let file = media_file(dir.path(), "clip.mkv", "clip.mkv");
    let handed = PaneRequest::OpenElsewhere(vec![FilePath::new(&file).unwrap()]);
    let player: Arc<dyn MediaHost> = Arc::new(Plays);
    /// Name, the host's player, the media stage shows, the pane hands the file to its host.
    type Row = (&'static str, Option<Arc<dyn MediaHost>>, usize, bool);
    let rows: Vec<Row> = vec![
        ("with a player it plays here", Some(player), 1, false),
        ("with none it is the host's to open", None, 0, true),
    ];
    for (name, media, stage, elsewhere) in rows {
        let host = open_in_a_pane(&file, media);
        assert_eq!(host.harness.count(".viewer-media"), stage, "{name}: stage");
        assert_eq!(host.asked(&handed), elsewhere, "{name}: handed to the host");
    }
}

/// Two panes in one window playing a video with the person's own mpv, through the host's player as
/// a real host builds it: one hub (`MediaHub::standalone`, one source of sound at a time) lent to
/// the edge as a `PlayerHost`. Needs mpv, mpv-wgpu's C plugin and a graphics adapter; skips saying
/// so where any is missing.
#[cfg(feature = "player")]
mod real_mpv {
    use super::in_a_corner;
    use crate::support::{hosted_with, media_file, mpv_host};
    use anyview_core::MediaTime;
    use anyview_pane::{AudioDriver, FixedMpv, MediaHub, PlayerHost};
    use anyview_ui::{
        MediaHost, MediaLine, MediaNotice, MediaPlayback, MediaStart, MediaStarted, OpenError,
        Pace, PlayerCommand, PlayerEvent, SlotPixels,
    };
    use ds_blitz::TextureHandle;
    use ds_harness::{Query, Viewport};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    /// How long a step may take on a machine with a software GPU.
    const STEP: Duration = Duration::from_secs(30);

    /// A pane's line to its player, remembering what the player said to the pane.
    #[derive(Debug)]
    struct Tap {
        line: Arc<dyn MediaLine>,
        heard: Mutex<Vec<MediaNotice>>,
    }

    impl Tap {
        fn heard(&self) -> Vec<MediaNotice> {
            self.heard
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        /// How the player last said it is going: `None` before it said.
        fn pace(&self) -> Option<Pace> {
            self.heard()
                .iter()
                .filter_map(|notice| match notice {
                    MediaNotice::Player(PlayerEvent::Playback(pace)) => Some(*pace),
                    MediaNotice::Player(_)
                    | MediaNotice::Position(_)
                    | MediaNotice::Tracks(_)
                    | MediaNotice::Chapters(_)
                    | MediaNotice::Speed(_)
                    | MediaNotice::Abilities(_)
                    | MediaNotice::Picture(_)
                    | MediaNotice::Failed(_)
                    | MediaNotice::Next
                    | MediaNotice::Previous => None,
                })
                .next_back()
        }

        fn has(&self, wanted: impl Fn(&MediaNotice) -> bool) -> bool {
            self.heard().iter().any(wanted)
        }

        /// What the person's Play does: from the start, so a three-second clip is not over.
        fn play(&self) {
            self.line.send(PlayerCommand::Seek(MediaTime::from_secs(0)));
            self.line.send(PlayerCommand::SetPlayback(Pace::Playing));
        }

        fn pause(&self) {
            self.line.send(PlayerCommand::SetPlayback(Pace::Paused));
        }
    }

    impl MediaLine for Tap {
        fn send(&self, command: PlayerCommand) {
            self.line.send(command);
        }

        fn resize(&self, slot: Option<SlotPixels>) {
            self.line.resize(slot);
        }

        fn drain(&self) -> Vec<MediaNotice> {
            let notices = self.line.drain();
            self.heard
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(notices.iter().cloned());
            notices
        }
    }

    /// A started player as the test holds it: what the pane heard and the texture it draws into.
    #[derive(Debug)]
    struct Session {
        tap: Arc<Tap>,
        texture: TextureHandle,
    }

    /// The host's player with a record of each player it started, in the order the panes asked.
    #[derive(Debug)]
    struct Watching {
        player: PlayerHost,
        sessions: Arc<Mutex<Vec<Arc<Session>>>>,
    }

    impl MediaHost for Watching {
        fn start(&self, start: MediaStart) -> Result<MediaStarted, OpenError> {
            let texture = start.texture.clone();
            let mut started = self.player.start(start)?;
            if let MediaPlayback::Line(line) = &started.playback {
                let tap = Arc::new(Tap {
                    line: Arc::clone(line),
                    heard: Mutex::new(Vec::new()),
                });
                started.playback = MediaPlayback::Line(tap.clone());
                self.sessions
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(Arc::new(Session { tap, texture }));
            }
            Ok(started)
        }
    }

    /// Run the window in real time, because the players run on threads of their own, until `done`.
    fn until(host: &mut crate::support::Host, what: &str, done: impl Fn() -> bool) {
        let started = Instant::now();
        while !done() {
            assert!(started.elapsed() < STEP, "{what} did not happen in time");
            host.harness.advance(Duration::from_millis(10));
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn two_panes_draw_and_sound_one_at_a_time_with_the_real_mpv() {
        let Some(programs) = mpv_host() else {
            return;
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let hub = MediaHub::standalone(
            runtime.handle(),
            AudioDriver::Null,
            Arc::new(FixedMpv::new(programs)),
        );
        let sessions = Arc::new(Mutex::new(Vec::new()));
        let watching = Watching {
            player: PlayerHost::new(hub),
            sessions: sessions.clone(),
        };
        let dir = tempfile::tempdir().unwrap();
        let files = ["left.mkv", "right.mkv"].map(|name| media_file(dir.path(), name, "clip.mkv"));
        let viewport = Viewport {
            width: 2 * in_a_corner().width,
            ..in_a_corner()
        };
        let mut host = hosted_with(
            files.into_iter().map(|file| (file, None)).collect(),
            viewport,
            None,
            Some(Arc::new(watching)),
        );
        let Some(window_device) = host.harness.gpu().and_then(|gpu| gpu.device()) else {
            crate::skip::skipped("the harness has no graphics adapter for the panes to draw on");
            return;
        };
        let both = || {
            let all = sessions.lock().unwrap_or_else(PoisonError::into_inner);
            (all.len() == 2).then(|| [Arc::clone(&all[0]), Arc::clone(&all[1])])
        };
        until(&mut host, "both panes to start a player", || {
            both().is_some()
        });
        let [first, second] = both().unwrap();

        // Each pane's picture reaches its own texture, and both are on the window's one device.
        until(&mut host, "a picture in each pane's texture", || {
            first.texture.size().is_some() && second.texture.size().is_some()
        });
        for (name, session) in [("first", &first), ("second", &second)] {
            assert_eq!(
                session.texture.gpu().device().as_ref(),
                Some(&window_device),
                "{name}: the pane's texture is on the window's device"
            );
        }
        assert_eq!(
            host.harness.count(".viewer-media"),
            2,
            "both panes show the stage"
        );

        // Both start playing on their own and the hub holds one of them. Hold both, then play the
        // first and then the second: starting the second is what pauses the first, with no
        // command to the first.
        until(&mut host, "both players to load", || {
            [&first, &second].iter().all(|session| {
                session
                    .tap
                    .has(|n| matches!(n, MediaNotice::Player(PlayerEvent::Loaded { .. })))
            })
        });
        first.tap.pause();
        second.tap.pause();
        until(&mut host, "both players to be held", || {
            first.tap.pace() == Some(Pace::Paused) && second.tap.pace() == Some(Pace::Paused)
        });
        first.tap.play();
        until(&mut host, "the first to play", || {
            first.tap.pace() == Some(Pace::Playing)
        });
        second.tap.play();
        until(&mut host, "the second to play", || {
            second.tap.pace() == Some(Pace::Playing)
        });
        until(&mut host, "the hub to pause the first", || {
            first.tap.pace() == Some(Pace::Paused)
        });
        assert_eq!(
            second.tap.pace(),
            Some(Pace::Playing),
            "the one that sounds last keeps playing"
        );
        assert!(
            !first.tap.has(|n| matches!(
                n,
                MediaNotice::Player(PlayerEvent::Ended(_)) | MediaNotice::Failed(_)
            )),
            "the first was paused by the hub, not because it ended or failed"
        );
    }
}
