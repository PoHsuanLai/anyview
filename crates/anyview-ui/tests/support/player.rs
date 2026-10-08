//! A player that is a script: the window starts it, the test says what it reports and reads what
//! the window told it, and a picture it uploads stands for the video.
#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::{Fact, FactLabel, FactValue, FilePath, Helper, MediaExportKind, MediaTags};
use anyview_ui::{
    MediaHost, MediaLine, MediaNotice, MediaOffer, MediaPlayback, MediaStart, MediaStarted,
    MediaWake, Need, OpenError, PlayerCommand, SlotPixels,
};
use ds_blitz::{PixelFormat, Pixels};
use ds_core::word::Word;
use std::sync::{Arc, Mutex, Weak};

/// One started player.
#[derive(Debug)]
pub struct FakeLine {
    sent: Mutex<Vec<PlayerCommand>>,
    slots: Mutex<Vec<Option<SlotPixels>>>,
    news: Mutex<Vec<MediaNotice>>,
    wake: MediaWake,
    pub file: FilePath,
}

impl FakeLine {
    /// Report `notices` as the player would, and wake the window.
    pub fn say(&self, notices: &[MediaNotice]) {
        self.news.lock().unwrap().extend(notices.iter().cloned());
        self.wake.wake();
    }

    /// What the window told the player, in order.
    pub fn sent(&self) -> Vec<PlayerCommand> {
        self.sent.lock().unwrap().clone()
    }

    /// The slot sizes the window asked for.
    pub fn slots(&self) -> Vec<Option<SlotPixels>> {
        self.slots.lock().unwrap().clone()
    }
}

impl MediaLine for FakeLine {
    fn send(&self, command: PlayerCommand) {
        self.sent.lock().unwrap().push(command);
    }

    fn resize(&self, slot: Option<SlotPixels>) {
        self.slots.lock().unwrap().push(slot);
    }

    fn drain(&self) -> Vec<MediaNotice> {
        std::mem::take(&mut *self.news.lock().unwrap())
    }
}

/// How the fake host answers the next start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Answer {
    /// A player starts, and the window's picture is a gradient.
    #[default]
    Plays,
    /// The player cannot be made.
    Refuses,
    /// No plugin plays recordings: the file opens as its facts and the package that would play it.
    Missing,
    /// The plugin is there and the system's mpv is not: the same card, with mpv to install.
    Lacks,
}

/// Starts [`FakeLine`]s and remembers them, weakly: a player the window let go of is gone.
#[derive(Debug, Default)]
pub struct FakePlayer {
    started: Mutex<Vec<Weak<FakeLine>>>,
    answer: Mutex<Answer>,
    tags: Mutex<MediaTags>,
    offer: Mutex<Option<MediaOffer>>,
    cover: Mutex<Option<ds::components::content::image_source::ImageSource>>,
}

impl FakePlayer {
    /// Answer `answer` from now on: what the person installed in the meantime.
    pub fn answer_from_now(&self, answer: Answer) {
        *self.answer.lock().unwrap() = answer;
    }

    pub fn answering(answer: Answer) -> Arc<FakePlayer> {
        let player = FakePlayer::default();
        *player.answer.lock().unwrap() = answer;
        Arc::new(player)
    }

    /// What the next player says its file is called.
    pub fn tagged(self: &Arc<Self>, tags: MediaTags) -> Arc<FakePlayer> {
        *self.tags.lock().unwrap() = tags;
        Arc::clone(self)
    }

    /// The cover the next player says its file carries.
    pub fn covered(
        self: &Arc<Self>,
        cover: ds::components::content::image_source::ImageSource,
    ) -> Arc<FakePlayer> {
        *self.cover.lock().unwrap() = Some(cover);
        Arc::clone(self)
    }

    /// The exports the host offers for what it starts; every media kind when none is set.
    pub fn offering(self: &Arc<Self>, offer: MediaOffer) -> Arc<FakePlayer> {
        *self.offer.lock().unwrap() = Some(offer);
        Arc::clone(self)
    }

    /// How many players were started, living or not.
    pub fn starts(&self) -> usize {
        self.started.lock().unwrap().len()
    }

    /// The players still held by a window, oldest first.
    pub fn alive(&self) -> Vec<Arc<FakeLine>> {
        self.started
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }

    /// The newest player still held.
    pub fn latest(&self) -> Option<Arc<FakeLine>> {
        self.alive().pop()
    }
}

/// A 64 by 36 gradient, so a frame of the video is something a person can see.
fn gradient() -> Vec<u8> {
    (0..36_u32)
        .flat_map(|y| {
            (0..64_u32).flat_map(move |x| {
                let (r, g) = (x * 4, y * 7);
                [r as u8, g as u8, 160, 255]
            })
        })
        .collect()
}

impl MediaHost for FakePlayer {
    fn start(&self, start: MediaStart) -> Result<MediaStarted, OpenError> {
        let offer = self
            .offer
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| MediaOffer::new(MediaExportKind::ALL.to_vec(), None));
        let answer = *self.answer.lock().unwrap();
        match answer {
            Answer::Plays => {}
            Answer::Refuses => return Err(OpenError::Media("no player".to_owned())),
            Answer::Missing | Answer::Lacks => {
                let fact = Fact {
                    label: FactLabel::Needs,
                    value: FactValue::text("anyview-mpv (to play it)"),
                };
                let need = match answer {
                    Answer::Lacks => Need {
                        fact,
                        helper: Some(Helper::VideoPlayback),
                    },
                    Answer::Plays | Answer::Refuses | Answer::Missing => Need::passive(fact),
                };
                return Ok(MediaStarted {
                    playback: MediaPlayback::Missing(need),
                    offer,
                    tags: self.tags.lock().unwrap().clone(),
                    facts: anyview_core::Facts::empty()
                        .with(FactLabel::Codec, FactValue::text("h264")),
                    length: None,
                    cover: None,
                });
            }
        }
        let bytes = gradient();
        if let Ok(pixels) = Pixels::new(PixelFormat::Rgba8Premultiplied, 64, 36, &bytes) {
            let _ = start.texture.update(&pixels);
        }
        let line = Arc::new(FakeLine {
            sent: Mutex::default(),
            slots: Mutex::default(),
            news: Mutex::default(),
            wake: start.wake,
            file: start.file,
        });
        self.started.lock().unwrap().push(Arc::downgrade(&line));
        Ok(MediaStarted {
            playback: MediaPlayback::Line(line),
            offer,
            tags: self.tags.lock().unwrap().clone(),
            facts: anyview_core::Facts::empty(),
            length: None,
            cover: self.cover.lock().unwrap().clone(),
        })
    }
}
