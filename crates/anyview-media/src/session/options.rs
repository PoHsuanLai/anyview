//! What a session is made with.

use ds_core::word::Word;
use mpv_wgpu_player::AudioOutput;

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
