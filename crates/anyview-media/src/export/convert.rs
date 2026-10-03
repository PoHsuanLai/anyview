//! Audio conversion: the best audio track decoded, resampled to what the encoder takes, encoded and
//! written to a container. Every format has its own sample format and rates, set out in `shape`.

use super::encoders::{AudioFormat, Encoders};
use super::fifo::SampleFifo;
use super::gate::Reporter;
use crate::MediaError;
use crate::libav::{av_time, libav, signed_micros, utf8};
use anyview_core::work::{Stop, StopState};
use anyview_core::{AudioTarget, MediaLength, MediaTime, TimeRange};
use ds_core::word::Word;
use ff::ChannelLayout;
use ff::format::Sample;
use ff::format::sample::Type as Layout;
use ff::media::Type;
use ffmpeg_next as ff;
use std::path::Path;

/// What an encoder is given.
struct Shape {
    format: Sample,
    rate: u32,
    layout: ChannelLayout,
}

/// The rates LAME encodes at.
const MP3_RATES: &[u32] = &[
    8000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];

/// The sample format, rate and layout `format` is encoded in, given what the source has: PCM and
/// FLAC keep the source's rate and layout, the lossy encoders take at most two channels and the
/// rates they support.
fn shape(format: AudioFormat, rate: u32, layout: ChannelLayout) -> Shape {
    let stereo = ChannelLayout::default(2);
    let lossy_layout = if layout.channels() > 2 {
        stereo
    } else {
        layout
    };
    match format {
        AudioFormat::M4a => Shape {
            format: Sample::F32(Layout::Planar),
            rate,
            layout: lossy_layout,
        },
        AudioFormat::Mp3 => Shape {
            format: Sample::I16(Layout::Planar),
            rate: if MP3_RATES.contains(&rate) {
                rate
            } else {
                44_100
            },
            layout: lossy_layout,
        },
        AudioFormat::Flac | AudioFormat::Wav => Shape {
            format: Sample::I16(Layout::Packed),
            rate,
            layout,
        },
        AudioFormat::Opus => Shape {
            format: Sample::I16(Layout::Packed),
            rate: 48_000,
            layout: lossy_layout,
        },
    }
}

/// Convert the best audio track of `source` to `target` in `to`.
pub(super) fn convert(
    source: &Path,
    to: &Path,
    range: TimeRange,
    target: AudioTarget,
    encoders: &Encoders,
    reporter: &mut Reporter,
    stop: &Stop,
) -> Result<MediaLength, MediaError> {
    let (Some(encoder_name), Some(format)) = (encoders.choose(target)?, AudioFormat::of(target))
    else {
        return Err(MediaError::NotMedia);
    };
    let mut input = ff::format::input(utf8(source)?).map_err(libav)?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .ok_or(MediaError::NoAudio)?;
    let (index, in_base) = (stream.index(), stream.time_base());
    let mut decoder = ff::codec::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().audio())
        .map_err(libav)?;
    let layout = match decoder.channel_layout() {
        empty if empty.is_empty() => ChannelLayout::default(i32::from(decoder.channels().max(1))),
        known => known,
    };
    decoder.set_channel_layout(layout);
    let shape = shape(format, decoder.rate(), layout);

    let codec = ff::encoder::find_by_name(encoder_name)
        .ok_or(MediaError::EncoderMissing(format.label()))?;
    let mut output = ff::format::output(utf8(to)?).map_err(libav)?;
    let wants_global_header = output
        .format()
        .flags()
        .contains(ff::format::Flags::GLOBAL_HEADER);
    let mut ost = output.add_stream(codec).map_err(libav)?;
    let mut writer = ff::codec::Context::new_with_codec(codec)
        .encoder()
        .audio()
        .map_err(libav)?;
    writer.set_rate(i32::try_from(shape.rate).unwrap_or(i32::MAX));
    writer.set_format(shape.format);
    writer.set_channel_layout(shape.layout);
    let out_base = ff::Rational::new(1, i32::try_from(shape.rate).unwrap_or(i32::MAX));
    writer.set_time_base(out_base);
    if let Some(rate) = bitrate_of(target) {
        writer.set_bit_rate(rate);
    }
    if wants_global_header {
        writer.set_flags(ff::codec::Flags::GLOBAL_HEADER);
    }
    if encoder_name == "opus" {
        // libav's own Opus encoder is marked experimental and refuses to run otherwise.
        writer.compliance(ff::codec::Compliance::Experimental);
    }
    let mut encoder = writer.open_as(codec).map_err(libav)?;
    ost.set_parameters(&encoder);
    ost.set_time_base(out_base);
    output.set_metadata(tags_of(&input));
    output.write_header().map_err(libav)?;
    let out_base = output
        .stream(0)
        .map_or(out_base, |written| written.time_base());

    let resampler = decoder
        .resampler(shape.format, shape.layout, shape.rate)
        .map_err(libav)?;
    let mut fifo = SampleFifo::new(shape.format, shape.layout, shape.rate);
    let frame_size = usize::try_from(encoder.frame_size()).unwrap_or(0);
    let start = av_time(range.start());
    if start > 0 {
        input.seek(start, ..start).map_err(libav)?;
    }
    let mut sink = Sink {
        encoder: &mut encoder,
        output: &mut output,
        out_base,
        encoded: 0,
        reporter,
        shape: &shape,
    };
    let mut pump = Pump {
        decoder,
        resampler,
        frame: ff::frame::Audio::empty(),
        layout,
        shape: &shape,
        in_base,
        start,
        end: range.end().map(av_time),
        frame_size,
    };
    for (stream, packet) in input.packets() {
        if stop.stopped() == StopState::Stopped {
            return Err(MediaError::Stopped);
        }
        if stream.index() != index {
            continue;
        }
        pump.decoder.send_packet(&packet).map_err(libav)?;
        if pump.drain(&mut sink, &mut fifo)? == Reach::Past {
            break;
        }
    }
    pump.decoder.send_eof().map_err(libav)?;
    pump.drain(&mut sink, &mut fifo)?;
    pump.flush_resampler(&mut fifo)?;
    sink.encode_ready(&mut fifo, frame_size)?;
    sink.encode_rest(&mut fifo)?;
    sink.finish()?;
    let length = MediaLength(sink.length(shape.rate));
    let samples = sink.encoded_samples();
    sink.reporter.finish(length.0);
    output.write_trailer().map_err(libav)?;
    if format == AudioFormat::Flac {
        super::flac::set_total(to, samples)?;
    }
    Ok(length)
}

/// Whether the decoded audio has reached the end of the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Within,
    Past,
}

/// The decoder and resampler, turning packets into queued samples in the encoder's shape.
struct Pump<'a> {
    decoder: ff::decoder::Audio,
    resampler: ff::software::resampling::Context,
    frame: ff::frame::Audio,
    layout: ChannelLayout,
    shape: &'a Shape,
    in_base: ff::Rational,
    start: i64,
    end: Option<i64>,
    frame_size: usize,
}

impl Pump<'_> {
    /// Take every frame the decoder has, drop those outside the range, convert the rest into the
    /// queue and encode the whole frames it then holds.
    fn drain(&mut self, sink: &mut Sink<'_>, fifo: &mut SampleFifo) -> Result<Reach, MediaError> {
        let mut reach = Reach::Within;
        while self.decoder.receive_frame(&mut self.frame).is_ok() {
            let time = self
                .frame
                .pts()
                .map_or(self.start, |pts| signed_micros(pts, self.in_base));
            if time < self.start {
                continue;
            }
            if self.end.is_some_and(|end| time >= end) {
                reach = Reach::Past;
                continue;
            }
            if self.frame.channel_layout().is_empty() {
                self.frame.set_channel_layout(self.layout);
            }
            let mut resampled = roomy(self.shape, self.frame.samples(), self.decoder.rate());
            self.resampler
                .run(&self.frame, &mut resampled)
                .map_err(libav)?;
            fifo.push(&resampled);
            sink.encode_ready(fifo, self.frame_size)?;
        }
        Ok(reach)
    }

    /// Queue what the resampler still holds once the decoder has nothing more.
    fn flush_resampler(&mut self, fifo: &mut SampleFifo) -> Result<(), MediaError> {
        loop {
            let mut tail = roomy(self.shape, 8192, self.shape.rate);
            let more = self.resampler.flush(&mut tail).map_err(libav)?;
            if tail.samples() > 0 {
                fifo.push(&tail);
            }
            if more.is_none() || tail.samples() == 0 {
                return Ok(());
            }
        }
    }
}

/// The bits a second a lossy target spends, as the encoder is told.
fn bitrate_of(target: AudioTarget) -> Option<usize> {
    match target {
        AudioTarget::M4a(rate) | AudioTarget::Mp3(rate) | AudioTarget::Opus(rate) => {
            usize::try_from(rate.bits_per_second()).ok()
        }
        AudioTarget::Copy | AudioTarget::Flac | AudioTarget::Wav => None,
    }
}

/// The title, artist and album the source carries, for the output to carry too.
fn tags_of(input: &ff::format::context::Input) -> ff::Dictionary<'static> {
    let mut tags = ff::Dictionary::new();
    let metadata = input.metadata();
    for key in ["title", "artist", "album"] {
        if let Some(value) = metadata.get(key) {
            tags.set(key, value);
        }
    }
    tags
}

/// An empty frame in the encoder's shape with room for the `samples` of a source at `rate` after
/// resampling, plus what the resampler holds back: the converter writes into it and says how many.
fn roomy(shape: &Shape, samples: usize, rate: u32) -> ff::frame::Audio {
    let wanted = samples * usize::try_from(shape.rate).unwrap_or(1)
        / usize::try_from(rate.max(1)).unwrap_or(1);
    let mut frame = ff::frame::Audio::new(shape.format, wanted + 4096, shape.layout);
    frame.set_rate(shape.rate);
    frame
}

/// Where encoded packets go.
struct Sink<'a> {
    encoder: &'a mut ff::encoder::Audio,
    output: &'a mut ff::format::context::Output,
    out_base: ff::Rational,
    /// How many samples have been given to the encoder: the next frame's time.
    encoded: i64,
    reporter: &'a mut Reporter,
    shape: &'a Shape,
}

impl Sink<'_> {
    /// Encode every whole frame the queue holds; with a variable frame size, everything it holds.
    fn encode_ready(&mut self, fifo: &mut SampleFifo, frame_size: usize) -> Result<(), MediaError> {
        let size = if frame_size == 0 {
            fifo.len().max(1)
        } else {
            frame_size
        };
        while fifo.len() >= size && fifo.len() > 0 {
            let frame = fifo.pop(size);
            self.send(frame)?;
        }
        Ok(())
    }

    /// Encode the last, short frame.
    fn encode_rest(&mut self, fifo: &mut SampleFifo) -> Result<(), MediaError> {
        if fifo.len() > 0 {
            let frame = fifo.pop(fifo.len());
            self.send(frame)?;
        }
        Ok(())
    }

    fn send(&mut self, mut frame: ff::frame::Audio) -> Result<(), MediaError> {
        frame.set_pts(Some(self.encoded));
        self.encoded += i64::try_from(frame.samples()).unwrap_or(0);
        self.encoder.send_frame(&frame).map_err(libav)?;
        self.drain()?;
        let time = self.length(self.shape.rate);
        self.reporter.at(time);
        Ok(())
    }

    /// Tell the encoder there is no more and write what it still holds.
    fn finish(&mut self) -> Result<(), MediaError> {
        self.encoder.send_eof().map_err(libav)?;
        self.drain()
    }

    fn drain(&mut self) -> Result<(), MediaError> {
        let mut packet = ff::Packet::empty();
        loop {
            match self.encoder.receive_packet(&mut packet) {
                // The empty packet an encoder ends with has nothing to write: the binding refuses
                // it, so what it carries (a FLAC encoder's final header) is patched in after
                // the trailer (`flac::set_total`).
                Ok(()) if packet.size() == 0 => {}
                Ok(()) => {
                    packet.set_stream(0);
                    packet.rescale_ts(
                        ff::Rational::new(1, i32::try_from(self.shape.rate).unwrap_or(1)),
                        self.out_base,
                    );
                    packet.write_interleaved(self.output).map_err(libav)?;
                }
                Err(ff::Error::Other { errno }) if errno == ff::util::error::EAGAIN => {
                    return Ok(());
                }
                Err(ff::Error::Eof) => return Ok(()),
                Err(other) => return Err(libav(other)),
            }
        }
    }

    /// How many samples have been encoded.
    fn encoded_samples(&self) -> u64 {
        u64::try_from(self.encoded).unwrap_or(0)
    }

    /// How much audio has been encoded.
    fn length(&self, rate: u32) -> MediaTime {
        let samples = u64::try_from(self.encoded).unwrap_or(0);
        MediaTime(samples * 1_000_000 / u64::from(rate.max(1)))
    }
}
