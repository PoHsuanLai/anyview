//! The machine's sound card, through cpal (ALSA on Linux, where PipeWire and PulseAudio answer on
//! the `default` device too).

use super::{Pipe, SoundOutput, StreamFormat};
use crate::error::MediaError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
};
use std::sync::Arc;

/// Whether the machine has a sound card to play on.
pub fn card_present() -> bool {
    cpal::default_host().default_output_device().is_some()
}

/// The default output device, opened as a stream the pipe is drained into.
#[derive(Default)]
pub struct CardOutput {
    device: Option<Device>,
    chosen: Option<SupportedStreamConfig>,
    stream: Option<Stream>,
}

impl std::fmt::Debug for CardOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardOutput").finish_non_exhaustive()
    }
}

/// How well a sample format suits: the card's own is converted from floats, so the order is how
/// little a conversion costs. `None` is a format the player cannot write.
fn rank(format: SampleFormat) -> Option<u8> {
    match format {
        SampleFormat::F32 => Some(0),
        SampleFormat::I32 => Some(1),
        SampleFormat::I16 => Some(2),
        SampleFormat::U16 => Some(3),
        _ => None,
    }
}

/// The configuration to open: the recording's own rate and channels when the card has them, which
/// spares a conversion, else the card's default.
fn choose(device: &Device, wanted: StreamFormat) -> Result<SupportedStreamConfig, MediaError> {
    let ranges = device
        .supported_output_configs()
        .map_err(|error| MediaError::SoundOutput(error.to_string()))?;
    let exact = ranges
        .filter(|range| range.channels() == wanted.channels.get())
        .filter_map(|range| {
            let format = rank(range.sample_format())?;
            Some((format, range.try_with_sample_rate(wanted.rate.get())?))
        })
        .min_by_key(|(format, _)| *format)
        .map(|(_, config)| config);
    if let Some(config) = exact {
        return Ok(config);
    }
    let fallback = device
        .default_output_config()
        .map_err(|error| MediaError::SoundOutput(error.to_string()))?;
    match rank(fallback.sample_format()) {
        Some(_) => Ok(fallback),
        None => Err(MediaError::SoundOutput(
            "the sound card takes no sample format the player writes".to_owned(),
        )),
    }
}

/// A stream of `T` samples that `pipe` fills; the card's own failure is the pipe's.
fn build<T>(device: &Device, config: &StreamConfig, pipe: Arc<Pipe>) -> Result<Stream, MediaError>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let mut floats: Vec<f32> = Vec::new();
    let failing = Arc::clone(&pipe);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _info| {
                floats.resize(data.len(), 0.0);
                pipe.fill(&mut floats);
                for (slot, sample) in data.iter_mut().zip(&floats) {
                    *slot = T::from_sample(*sample);
                }
            },
            move |error| failing.fail(error.to_string()),
            None,
        )
        .map_err(|error| MediaError::SoundOutput(error.to_string()))
}

impl SoundOutput for CardOutput {
    fn negotiate(&mut self, wanted: StreamFormat) -> Result<StreamFormat, MediaError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(MediaError::NoSoundOutput)?;
        let chosen = choose(&device, wanted)?;
        let granted = StreamFormat::new(chosen.sample_rate(), chosen.channels())
            .ok_or_else(|| MediaError::SoundOutput("the sound card has no format".to_owned()))?;
        self.device = Some(device);
        self.chosen = Some(chosen);
        Ok(granted)
    }

    fn start(&mut self, pipe: Arc<Pipe>) -> Result<(), MediaError> {
        let (Some(device), Some(chosen)) = (&self.device, &self.chosen) else {
            return Err(MediaError::NoSoundOutput);
        };
        let config = chosen.config();
        let stream = match chosen.sample_format() {
            SampleFormat::F32 => build::<f32>(device, &config, pipe)?,
            SampleFormat::I32 => build::<i32>(device, &config, pipe)?,
            SampleFormat::I16 => build::<i16>(device, &config, pipe)?,
            SampleFormat::U16 => build::<u16>(device, &config, pipe)?,
            _ => {
                return Err(MediaError::SoundOutput(
                    "an unsupported sample format".to_owned(),
                ));
            }
        };
        stream
            .play()
            .map_err(|error| MediaError::SoundOutput(error.to_string()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn pause(&mut self) {
        if let Some(stream) = &self.stream {
            // A device that cannot pause goes on asking for sound, and the pipe's shut gate
            // answers it with silence: the pause still holds.
            let _unsupported = stream.pause();
        }
    }

    fn resume(&mut self) {
        if let Some(stream) = &self.stream {
            let _failed = stream.play();
        }
    }
}
