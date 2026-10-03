//! What a session is made with.

use ds_core::word::Word;
use mpv_wgpu_player::{AudioOutput, Host, SubprocessOptions};
use std::path::PathBuf;

/// The programs a session runs: the person's own `mpv` and the C plugin of mpv-wgpu that is loaded
/// into it. They come from a `play` entry of a plugin manifest; nothing here looks for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvHost {
    /// The stock player, an absolute path.
    pub mpv: PathBuf,
    /// The shared library loaded into it, an absolute path.
    pub cplugin: PathBuf,
}

impl MpvHost {
    pub(super) fn host(&self) -> Host {
        Host::Subprocess(SubprocessOptions {
            mpv: Some(self.mpv.clone()),
            cplugin: Some(self.cplugin.clone()),
            extra_args: Vec::new(),
        })
    }
}

/// Which audio driver plays the sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum AudioDriver {
    /// Let mpv probe the drivers it has.
    #[default]
    Auto,
    /// PulseAudio.
    Pulse,
    /// PipeWire.
    PipeWire,
    /// ALSA.
    Alsa,
    /// Decode and clock the sound, play nothing: for tests and headless runs.
    Null,
}

impl AudioDriver {
    /// The driver called `name` (`auto`, `pulse`, `pipewire`, `alsa`, `null`), if there is one.
    pub fn from_name(name: &str) -> Option<AudioDriver> {
        Word::parse(name)
    }

    pub(super) fn output(self) -> AudioOutput {
        match self {
            AudioDriver::Auto => AudioOutput::Auto,
            AudioDriver::Pulse => AudioOutput::Pulse,
            AudioDriver::PipeWire => AudioOutput::PipeWire,
            AudioDriver::Alsa => AudioOutput::Alsa,
            AudioDriver::Null => AudioOutput::Null,
        }
    }
}
