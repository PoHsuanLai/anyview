//! Stream copy: a recording cut, or its audio track lifted out, with no decode and no re-encode, so
//! it is instant and loses nothing. A cut can only start where a picture does not depend on an
//! earlier one, so it starts at the keyframe at or before the time asked for and keeps the
//! pictures that follow it. This writes a copy; saving a trim over its source goes through the
//! save pipeline, which keeps the original.

use super::gate::Reporter;
use crate::MediaError;
use crate::libav::{av_time, in_base, libav, signed_micros, utf8};
use anyview_core::work::{Stop, StopState};
use anyview_core::{MediaLength, MediaTime, StreamPick, TimeRange};
use ff::media::Type;
use ffmpeg_next as ff;
use std::collections::HashMap;
use std::path::Path;

/// How much of the recording after the end of the cut is read before giving up on the streams that
/// have not reached it: demuxers interleave within about this much.
const SLACK: i64 = 1_000_000;

/// The packets of the streams a cut keeps.
struct Cut {
    /// Input stream index to output stream index.
    map: HashMap<usize, usize>,
    /// The stream whose first keyframe is the start of the cut.
    lead: usize,
    /// Where the cut starts, in microseconds, once the lead stream has said.
    origin: Option<i64>,
    /// Where it ends, in microseconds.
    end: Option<i64>,
    /// What has been written, as the end of the last packet, in microseconds.
    written: i64,
}

/// Write `streams` of `source` between `range`'s ends into `to`, copying packets.
pub(super) fn copy(
    source: &Path,
    to: &Path,
    range: TimeRange,
    streams: StreamPick,
    reporter: &mut Reporter,
    stop: &Stop,
) -> Result<MediaLength, MediaError> {
    let mut input = ff::format::input(utf8(source)?).map_err(libav)?;
    let mut output = ff::format::output(utf8(to)?).map_err(libav)?;
    let mut cut = open_streams(&input, &mut output, streams, range)?;
    output.set_metadata(input.metadata().to_owned());
    add_chapters(&input, &mut output, range);
    let mut options = ff::Dictionary::new();
    options.set("avoid_negative_ts", "make_zero");
    output.write_header_with(options).map_err(libav)?;
    let out_bases: HashMap<usize, ff::Rational> = cut
        .map
        .iter()
        .filter_map(|(from, to)| output.stream(*to).map(|s| (*from, s.time_base())))
        .collect();
    let in_bases: HashMap<usize, ff::Rational> = cut
        .map
        .keys()
        .filter_map(|index| input.stream(*index).map(|s| (*index, s.time_base())))
        .collect();
    if range.start() > MediaTime(0) {
        let at = av_time(range.start());
        input.seek(at, ..at).map_err(libav)?;
    }
    let mut pending: Vec<ff::Packet> = Vec::new();
    let mut ctx = Context {
        output: &mut output,
        cut: &mut cut,
        in_bases,
        out_bases,
        reporter,
        start: av_time(range.start()),
    };
    for (stream, packet) in input.packets() {
        if stop.stopped() == StopState::Stopped {
            return Err(MediaError::Stopped);
        }
        let index = stream.index();
        if !ctx.cut.map.contains_key(&index) {
            continue;
        }
        if ctx.cut.origin.is_none() {
            let time = packet_micros(&packet, ctx.in_bases[&index]);
            if index == ctx.cut.lead && packet.is_key() {
                ctx.cut.origin = Some(time);
                for held in std::mem::take(&mut pending) {
                    ctx.write(held)?;
                }
            } else {
                pending.push(packet);
                continue;
            }
        }
        if ctx.write(packet)? == Flow::Done {
            break;
        }
    }
    // A recording with no keyframe after the seek has no origin: what was held starts at the
    // time asked for.
    if ctx.cut.origin.is_none() {
        ctx.cut.origin = Some(ctx.start);
        for held in pending {
            ctx.write(held)?;
        }
    }
    let written = ctx.cut.written;
    ctx.reporter
        .finish(MediaTime(u64::try_from(written).unwrap_or(0)));
    output.write_trailer().map_err(libav)?;
    Ok(MediaLength(MediaTime(u64::try_from(written).unwrap_or(0))))
}

/// Whether the cut has more to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    More,
    Done,
}

struct Context<'a> {
    output: &'a mut ff::format::context::Output,
    cut: &'a mut Cut,
    in_bases: HashMap<usize, ff::Rational>,
    out_bases: HashMap<usize, ff::Rational>,
    reporter: &'a mut Reporter,
    start: i64,
}

impl Context<'_> {
    /// Write `packet` shifted to the cut's origin, or drop it when it is before the origin or
    /// after the end.
    fn write(&mut self, mut packet: ff::Packet) -> Result<Flow, MediaError> {
        let index = packet.stream();
        let (in_base, out_base) = (self.in_bases[&index], self.out_bases[&index]);
        let origin = self.cut.origin.unwrap_or(self.start);
        let time = packet_micros(&packet, in_base);
        let slack_end = self.cut.end.map(|end| end.saturating_add(SLACK));
        if slack_end.is_some_and(|end| time >= end) {
            return Ok(Flow::Done);
        }
        let ended = self.cut.end.is_some_and(|end| time >= end);
        if time < origin || ended {
            return Ok(Flow::More);
        }
        let shift = in_base_units(origin, in_base);
        packet.set_pts(packet.pts().map(|pts| pts - shift));
        packet.set_dts(packet.dts().map(|dts| dts - shift));
        packet.rescale_ts(in_base, out_base);
        packet.set_stream(self.cut.map[&index]);
        let ends = time - origin + signed_micros(packet.duration(), in_base);
        self.cut.written = self.cut.written.max(ends);
        self.reporter
            .at(MediaTime(u64::try_from(time - origin).unwrap_or(0)));
        packet.write_interleaved(self.output).map_err(libav)?;
        Ok(Flow::More)
    }
}

fn in_base_units(micros: i64, base: ff::Rational) -> i64 {
    in_base(micros, base)
}

/// The time of a packet in microseconds: its presentation time, or its decode time when it has
/// none.
fn packet_micros(packet: &ff::Packet, base: ff::Rational) -> i64 {
    packet
        .pts()
        .or_else(|| packet.dts())
        .map_or(0, |time| signed_micros(time, base))
}

/// Add an output stream for each input stream kept and say which feeds which.
fn open_streams(
    input: &ff::format::context::Input,
    output: &mut ff::format::context::Output,
    pick: StreamPick,
    range: TimeRange,
) -> Result<Cut, MediaError> {
    let mut map = HashMap::new();
    let mut lead = None;
    let best_audio = input.streams().best(Type::Audio).map(|s| s.index());
    for stream in input.streams() {
        let parameters = stream.parameters();
        let medium = parameters.medium();
        let kept = match (medium, pick) {
            (Type::Video, StreamPick::Everything) => !stream
                .disposition()
                .contains(ff::format::stream::Disposition::ATTACHED_PIC),
            (Type::Audio | Type::Subtitle, StreamPick::Everything) => true,
            (Type::Audio, StreamPick::AudioOnly) => Some(stream.index()) == best_audio,
            (
                Type::Video | Type::Subtitle | Type::Unknown | Type::Data | Type::Attachment,
                StreamPick::AudioOnly,
            )
            | (Type::Unknown | Type::Data | Type::Attachment, StreamPick::Everything) => false,
        };
        if !kept {
            continue;
        }
        let mut made = output
            .add_stream(ff::encoder::find(ff::codec::Id::None))
            .map_err(libav)?;
        made.set_parameters(parameters);
        made.set_metadata(stream.metadata().to_owned());
        map.insert(stream.index(), made.index());
        // The cut starts at a picture's keyframe when there is a picture, and at the first sound
        // when there is not.
        match (lead, medium) {
            (None, _) | (Some((_, Type::Audio | Type::Subtitle)), Type::Video) => {
                lead = Some((stream.index(), medium));
            }
            (Some(_), _) => {}
        }
    }
    let (lead, _) = lead.ok_or(MediaError::NoAudio)?;
    Ok(Cut {
        map,
        lead,
        origin: None,
        end: range.end().map(av_time),
        written: 0,
    })
}

/// Carry the chapters that overlap the cut over, shifted to its start.
fn add_chapters(
    input: &ff::format::context::Input,
    output: &mut ff::format::context::Output,
    range: TimeRange,
) {
    let start = av_time(range.start());
    for chapter in input.chapters() {
        let base = chapter.time_base();
        let (from, to) = (
            signed_micros(chapter.start(), base),
            signed_micros(chapter.end(), base),
        );
        let ends = range.end().map(av_time);
        if to <= start || ends.is_some_and(|end| from >= end) {
            continue;
        }
        let from = (from.max(start) - start).max(0);
        let to = ends.map_or(to, |end| to.min(end)) - start;
        let title = chapter
            .metadata()
            .get("title")
            .unwrap_or_default()
            .to_owned();
        // A chapter is a nicety: one the muxer refuses does not fail the cut.
        let _ = output.add_chapter(
            chapter.id(),
            ff::Rational::new(1, 1_000_000),
            from,
            to.max(from),
            title,
        );
    }
}
