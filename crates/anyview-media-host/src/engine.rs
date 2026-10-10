//! Which player plays a recording: the person's mpv when a plugin has it, else the built-in audio
//! player when it can play the file, else the plugin's absence as a row that names the package. The
//! choice is a pure function of what was found; what is found (the file's codec, the sound card)
//! is asked once, beside it.

use super::guard::Guarded;
use super::plugins::PlayRoute;
use anyview_core::{Fact, FactLabel, FactValue, FormatKind};
use anyview_media::{AudioDriver, Device, FrameSink, MediaDriver, MediaError, MpvHost, Queue};
use anyview_plugin::MissingPlugin;
use std::path::Path;

/// Whether the built-in audio player can play a file here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BuiltinAbility {
    /// It decodes the file and the machine has a sound output.
    Plays,
    /// It decodes the file, and the machine has no sound output to play it on.
    NoSound,
    /// It does not decode the file (a video, Opus, a damaged stream), or it is not built in.
    CannotDecode,
}

/// Which player a recording gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Chosen {
    /// The person's mpv, with these programs.
    Mpv(MpvHost),
    /// The built-in audio player.
    Builtin,
    /// The built-in player could play it but the machine has no sound output.
    NoSound,
    /// None does; this package would.
    Missing(MissingPlugin),
    /// None does, and no package is known to.
    Unserved,
}

/// The player for a recording, from the plugin route and what the built-in player can do.
pub(super) fn choose(route: PlayRoute, builtin: BuiltinAbility) -> Chosen {
    match (route, builtin) {
        (PlayRoute::Ready(host), _) => Chosen::Mpv(host),
        (_, BuiltinAbility::Plays) => Chosen::Builtin,
        (_, BuiltinAbility::NoSound) => Chosen::NoSound,
        (PlayRoute::Missing(missing), BuiltinAbility::CannotDecode) => Chosen::Missing(missing),
        (PlayRoute::Unserved, BuiltinAbility::CannotDecode) => Chosen::Unserved,
    }
}

/// The row a recording shows in place of a player when the machine has no sound output.
pub(super) fn no_sound_fact() -> Fact {
    Fact::new(
        FactLabel::Needs,
        FactValue::text("a sound output (to play it)"),
    )
}

/// Whether the built-in player can play `file`, a recording of `kind`. Reads the start of the file.
#[cfg(feature = "audio")]
pub(super) fn builtin_ability(kind: FormatKind, file: &Path, audio: AudioDriver) -> BuiltinAbility {
    if kind != FormatKind::Audio || anyview_media::playable(file).is_err() {
        return BuiltinAbility::CannotDecode;
    }
    match audio {
        AudioDriver::Null => BuiltinAbility::Plays,
        AudioDriver::Auto | AudioDriver::Pulse | AudioDriver::PipeWire | AudioDriver::Alsa => {
            if anyview_media::card_present() {
                BuiltinAbility::Plays
            } else {
                BuiltinAbility::NoSound
            }
        }
    }
}

/// The built-in player is not built in: it plays nothing.
#[cfg(not(feature = "audio"))]
pub(super) fn builtin_ability(
    _kind: FormatKind,
    _file: &Path,
    _audio: AudioDriver,
) -> BuiltinAbility {
    BuiltinAbility::CannotDecode
}

/// What a player is built from, on the media thread that will own it.
pub(super) enum Engine {
    /// mpv, drawing into the window's device.
    Mpv {
        device: Device,
        queue: Queue,
        audio: AudioDriver,
        host: MpvHost,
        sink: Box<dyn FrameSink>,
    },
    /// The built-in audio player, on the sound output `audio` names.
    Builtin { audio: AudioDriver },
}

impl Engine {
    /// The driver this engine makes for `file`. `wake` is what the player calls, from any thread,
    /// to have the media thread look to it.
    pub(super) fn start(
        self,
        file: &anyview_core::FilePath,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Box<dyn MediaDriver>, MediaError> {
        match self {
            Engine::Mpv {
                device,
                queue,
                audio,
                host,
                sink,
            } => {
                let driver =
                    anyview_media::Driver::open(&device, &queue, audio, &host, file, sink, wake)?;
                Ok(Box::new(Guarded::new(driver)))
            }
            Engine::Builtin { audio } => builtin(audio, file, wake),
        }
    }
}

#[cfg(feature = "audio")]
fn builtin(
    audio: AudioDriver,
    file: &anyview_core::FilePath,
    wake: impl Fn() + Send + Sync + 'static,
) -> Result<Box<dyn MediaDriver>, MediaError> {
    use anyview_media::{BuiltinDriver, CardOutput, SilentOutput, SoundOutput};
    let output: Box<dyn SoundOutput> = match audio {
        AudioDriver::Null => Box::new(SilentOutput::default()),
        AudioDriver::Auto | AudioDriver::Pulse | AudioDriver::PipeWire | AudioDriver::Alsa => {
            Box::new(CardOutput::default())
        }
    };
    let driver = BuiltinDriver::open(output, file, wake)?;
    Ok(Box::new(Guarded::new(driver)))
}

#[cfg(not(feature = "audio"))]
fn builtin(
    _audio: AudioDriver,
    _file: &anyview_core::FilePath,
    _wake: impl Fn() + Send + Sync + 'static,
) -> Result<Box<dyn MediaDriver>, MediaError> {
    Err(MediaError::NoDecoder("audio".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_plugin::{Package, Subject, suggested_package};
    use anyview_plugin_protocol::Capability;

    fn host() -> MpvHost {
        MpvHost {
            mpv: "/usr/bin/mpv".into(),
            cplugin: "/usr/lib/plugin.so".into(),
        }
    }

    fn missing(kind: FormatKind) -> MissingPlugin {
        let subject = Subject { kind, mime: None };
        let package: Package = suggested_package(Capability::Play, &subject).unwrap();
        MissingPlugin {
            capability: Capability::Play,
            kind,
            package,
        }
    }

    #[test]
    fn mpv_plays_whenever_it_is_there_and_the_built_in_player_plays_what_it_can_when_it_is_not() {
        let audio = missing(FormatKind::Audio);
        /// Name, the plugin route, what the built-in player can do, the player chosen.
        type Case = (&'static str, PlayRoute, BuiltinAbility, Chosen);
        let cases: Vec<Case> = vec![
            (
                "mpv, though the built-in player could",
                PlayRoute::Ready(host()),
                BuiltinAbility::Plays,
                Chosen::Mpv(host()),
            ),
            (
                "mpv for what only it plays",
                PlayRoute::Ready(host()),
                BuiltinAbility::CannotDecode,
                Chosen::Mpv(host()),
            ),
            (
                "no mpv, a file the built-in player decodes",
                PlayRoute::Missing(audio),
                BuiltinAbility::Plays,
                Chosen::Builtin,
            ),
            (
                "no mpv and no plugin known, a file it decodes",
                PlayRoute::Unserved,
                BuiltinAbility::Plays,
                Chosen::Builtin,
            ),
            (
                "no mpv and no sound output",
                PlayRoute::Missing(audio),
                BuiltinAbility::NoSound,
                Chosen::NoSound,
            ),
            (
                "no mpv, Opus: the package that would play it",
                PlayRoute::Missing(audio),
                BuiltinAbility::CannotDecode,
                Chosen::Missing(audio),
            ),
            (
                "no mpv, a video: the package that would play it",
                PlayRoute::Missing(missing(FormatKind::Video)),
                BuiltinAbility::CannotDecode,
                Chosen::Missing(missing(FormatKind::Video)),
            ),
            (
                "nothing known to play it",
                PlayRoute::Unserved,
                BuiltinAbility::CannotDecode,
                Chosen::Unserved,
            ),
        ];
        for (name, route, ability, want) in cases {
            assert_eq!(choose(route, ability), want, "{name}");
        }
    }

    #[cfg(feature = "audio")]
    #[test]
    fn the_ability_follows_the_file_and_the_sound_output() {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../anyview-media/tests/fixtures");
        let peek =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../anyview-peek/tests/fixtures");
        /// Name, the kind, the file, what it can do with the sound output silent.
        type Case = (&'static str, FormatKind, std::path::PathBuf, BuiltinAbility);
        let cases: Vec<Case> = vec![
            (
                "an mp3",
                FormatKind::Audio,
                fixtures.join("sine.mp3"),
                BuiltinAbility::Plays,
            ),
            (
                "a flac",
                FormatKind::Audio,
                fixtures.join("tone.flac"),
                BuiltinAbility::Plays,
            ),
            (
                "an m4a",
                FormatKind::Audio,
                peek.join("song.m4a"),
                BuiltinAbility::Plays,
            ),
            (
                "opus has no decoder",
                FormatKind::Audio,
                fixtures.join("sine.opus"),
                BuiltinAbility::CannotDecode,
            ),
            (
                "a video is not played",
                FormatKind::Video,
                peek.join("clip.mp4"),
                BuiltinAbility::CannotDecode,
            ),
            (
                "a video's audio is not played either",
                FormatKind::Audio,
                peek.join("clip.mp4"),
                BuiltinAbility::CannotDecode,
            ),
            (
                "a file that is gone",
                FormatKind::Audio,
                fixtures.join("gone.mp3"),
                BuiltinAbility::CannotDecode,
            ),
        ];
        for (name, kind, file, want) in cases {
            assert_eq!(
                builtin_ability(kind, &file, AudioDriver::Null),
                want,
                "{name}"
            );
        }
    }

    #[test]
    fn no_sound_output_is_a_row_that_says_so() {
        let fact = no_sound_fact();
        assert_eq!(fact.label, FactLabel::Needs);
        assert_eq!(fact.value.as_str(), "a sound output (to play it)");
    }
}
