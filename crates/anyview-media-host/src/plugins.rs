//! What the host asks of the plugins when it starts a player: which programs play a recording, what
//! the file says of itself, which program writes its exports and what to say when one is missing.
//! The viewer answers from the plugins the person installed; a host with none (a pane in another
//! application) answers with [`FixedMpv`].

use anyview_core::{Facts, FormatKind, MediaTags, Sniffed, Source};
use anyview_media::MpvHost;
use anyview_platform::PluginRunner;
use anyview_plugin::{Installed, MissingPlugin, Subject};
use anyview_ui::{MediaOffer, Need};
use std::fmt::Debug;
use std::sync::Arc;

/// What asking for a player came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayRoute {
    /// A plugin plays it, with these programs.
    Ready(MpvHost),
    /// None does; this package would.
    Missing(MissingPlugin),
    /// None does, and no package is known to.
    Unserved,
}

/// A recording's facts and tags, from whoever could read them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    /// The rows beyond the kind and the size, which the window adds itself.
    pub facts: Facts,
    /// The title, artist and album the recording says.
    pub tags: MediaTags,
    /// The cover an audio file carries, as PNG bytes (a plugin's facts have no picture).
    pub cover: Option<Vec<u8>>,
}

/// Which program writes a recording's exports.
#[derive(Debug, Clone)]
pub struct ExportTool {
    /// The runner that talks to it.
    pub runner: PluginRunner,
    /// The plugin that writes.
    pub plugin: Installed,
}

/// What asking for a writer came to.
#[derive(Debug, Clone)]
pub enum WriteRoute {
    /// A plugin writes it.
    Ready(Arc<ExportTool>),
    /// None does; this package would.
    Missing(MissingPlugin),
    /// None does, and no package is known to.
    Unserved,
}

/// Whether a player shows the recording's picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    /// A player was started for it.
    Yes,
    /// None was.
    No,
}

/// The plugins a [`PlayerHost`](crate::PlayerHost) and its [`MediaHub`](crate::MediaHub) ask. The
/// viewer implements it over the plugins the person installed; [`FixedMpv`] is the answer of a host
/// that has one mpv and nothing else.
pub trait PlayerPlugins: Debug + Send + Sync + 'static {
    /// What the recording `source` says of itself. A recording that cannot be read still plays, so
    /// a failure is a reading with fewer rows.
    fn reading(&self, source: &Source, sniffed: &Sniffed) -> Reading;

    /// The programs that play a file of `subject`'s kind.
    fn player(&self, subject: &Subject<'_>) -> PlayRoute;

    /// The writer of `subject`'s exports.
    fn writer(&self, subject: &Subject<'_>) -> WriteRoute;

    /// The row that says what would play, or read, a recording `missing` names, with the tool to
    /// offer to install when there is one.
    fn need_of(&self, missing: &MissingPlugin) -> Need;

    /// The exports on offer for a recording of `sniffed`'s kind.
    fn offer(&self, sniffed: &Sniffed, playing: Playing) -> MediaOffer;
}

/// The plugins of a host that has the person's `mpv` and its C plugin and no plugin registry: it
/// plays audio and video, reads no facts, offers no export and installs nothing.
#[derive(Debug, Clone)]
pub struct FixedMpv {
    host: MpvHost,
}

impl FixedMpv {
    /// Plugins that play with `host`'s programs.
    #[must_use]
    pub fn new(host: MpvHost) -> FixedMpv {
        FixedMpv { host }
    }
}

impl PlayerPlugins for FixedMpv {
    fn reading(&self, _source: &Source, _sniffed: &Sniffed) -> Reading {
        Reading::default()
    }

    fn player(&self, subject: &Subject<'_>) -> PlayRoute {
        if matches!(subject.kind, FormatKind::Audio | FormatKind::Video) {
            PlayRoute::Ready(self.host.clone())
        } else {
            PlayRoute::Unserved
        }
    }

    fn writer(&self, _subject: &Subject<'_>) -> WriteRoute {
        WriteRoute::Unserved
    }

    fn need_of(&self, missing: &MissingPlugin) -> Need {
        Need::passive(missing.fact())
    }

    fn offer(&self, _sniffed: &Sniffed, _playing: Playing) -> MediaOffer {
        MediaOffer::new(Vec::new(), None)
    }
}
