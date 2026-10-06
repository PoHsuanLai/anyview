//! The built-in audio player: pure-Rust decoding (symphonia) played through the sound card
//! (cpal), for a machine with no mpv. It plays what symphonia decodes (MP3, AAC, ALAC, FLAC,
//! Vorbis, PCM in WAV and AIFF); a picture or Opus is mpv's.
//!
//! The decoder runs on whichever thread calls [`BuiltinDriver::woken`], the media thread, and the
//! sound card calls the [`Pipe`] from its own: they share one queue and nothing else.

mod convert;
mod driver;
mod format;
mod output;
mod pipe;
mod source;

pub use driver::{BuiltinDriver, playable};
pub use format::StreamFormat;
pub use output::{CardOutput, SilentOutput, SoundOutput, card_present};
pub use pipe::{Gate, Pipe};
