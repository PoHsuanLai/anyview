//! Making decoded sound fit the sound card: the channels it has and the rate it runs at. Pure:
//! samples in, samples out.

use super::format::StreamFormat;

/// Mixes `input` (interleaved frames of `from` channels) to `to` channels, appending to `out`.
/// Mono is spread to every channel; any layout down to mono is averaged; six channels (front pair,
/// centre, low-frequency, rear pair) fold to stereo with the centre and rear at 0.7 and 0.5;
/// otherwise the first channels are kept and the extra ones of the card are silent.
pub(super) fn remix(input: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
    if from == 0 || to == 0 {
        return;
    }
    if from == to {
        out.extend_from_slice(input);
        return;
    }
    for frame in input.chunks_exact(from) {
        match (from, to) {
            (1, _) => out.extend(std::iter::repeat_n(frame[0], to)),
            (_, 1) => out.push(frame.iter().sum::<f32>() / from as f32),
            (6, 2) => {
                let centre = 0.7 * frame[2];
                out.push(frame[0] + centre + 0.5 * frame[4]);
                out.push(frame[1] + centre + 0.5 * frame[5]);
            }
            _ => out.extend((0..to).map(|channel| frame.get(channel).copied().unwrap_or(0.0))),
        }
    }
}

/// A linear-interpolating resampler that keeps its place across the pieces it is given, so a
/// stream cut anywhere comes out as if it had been one piece.
#[derive(Debug, Clone)]
pub(super) struct Resampler {
    /// Input frames consumed per output frame.
    step: f64,
    /// Where the next output frame falls, counted in input frames from the start of the next piece
    /// (negative: between the last frame of the piece before and the first of this one).
    at: f64,
    /// The last frame of the piece before: the left neighbour of a position that is negative.
    last: Vec<f32>,
    width: usize,
}

impl Resampler {
    /// A resampler from `from` frames a second to `to`, over `width` channels.
    pub(super) fn new(from: u64, to: u64, width: usize) -> Resampler {
        Resampler {
            step: from as f64 / to as f64,
            at: 0.0,
            last: Vec::new(),
            width,
        }
    }

    /// Forget the piece before: the next one starts a new stretch (after a seek).
    pub(super) fn reset(&mut self) {
        self.at = 0.0;
        self.last.clear();
    }

    /// Resample `input` (interleaved) and append the result to `out`.
    pub(super) fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let width = self.width;
        let frames = input.len() / width.max(1);
        if frames == 0 {
            return;
        }
        if self.last.is_empty() {
            self.last = input[..width].to_vec();
        }
        let sample = |index: i64, channel: usize| -> f32 {
            match usize::try_from(index) {
                Ok(index) => input[index.min(frames - 1) * width + channel],
                Err(_) => self.last[channel],
            }
        };
        let mut produced = Vec::new();
        while self.at < frames as f64 - 1.0 {
            let left = self.at.floor();
            let fraction = (self.at - left) as f32;
            let index = left as i64;
            for channel in 0..width {
                let (a, b) = (sample(index, channel), sample(index + 1, channel));
                produced.push(a + (b - a) * fraction);
            }
            self.at += self.step;
        }
        out.extend_from_slice(&produced);
        self.at -= frames as f64;
        self.last = input[(frames - 1) * width..frames * width].to_vec();
    }
}

/// Everything between the decoder's format and the card's.
#[derive(Debug, Clone)]
pub(super) struct Conversion {
    from: StreamFormat,
    to: StreamFormat,
    resampler: Option<Resampler>,
    mixed: Vec<f32>,
}

impl Conversion {
    /// A conversion from `from` to `to`.
    pub(super) fn new(from: StreamFormat, to: StreamFormat) -> Conversion {
        let resampler = (from.rate != to.rate)
            .then(|| Resampler::new(from.frames_a_second(), to.frames_a_second(), to.width()));
        Conversion {
            from,
            to,
            resampler,
            mixed: Vec::new(),
        }
    }

    /// Convert `input`, which is in the decoder's format, and append it to `out` in the card's.
    pub(super) fn run(&mut self, input: &[f32], out: &mut Vec<f32>) {
        match &mut self.resampler {
            None => remix(input, self.from.width(), self.to.width(), out),
            Some(resampler) => {
                self.mixed.clear();
                remix(input, self.from.width(), self.to.width(), &mut self.mixed);
                resampler.process(&self.mixed, out);
            }
        }
    }

    /// Start a new stretch of sound (after a seek).
    pub(super) fn reset(&mut self) {
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_mixed_to_the_cards() {
        /// Name, channels in, channels out, input, output.
        type Case = (&'static str, usize, usize, &'static [f32], &'static [f32]);
        const CASES: &[Case] = &[
            ("same", 2, 2, &[0.1, 0.2], &[0.1, 0.2]),
            (
                "mono to stereo",
                1,
                2,
                &[0.5, 0.25],
                &[0.5, 0.5, 0.25, 0.25],
            ),
            ("stereo to mono", 2, 1, &[0.5, 0.25], &[0.375]),
            ("stereo to four", 2, 4, &[0.5, 0.25], &[0.5, 0.25, 0.0, 0.0]),
            (
                "four to stereo keeps the front",
                4,
                2,
                &[1.0, 2.0, 3.0, 4.0],
                &[1.0, 2.0],
            ),
            (
                "five point one folds down",
                6,
                2,
                &[1.0, 0.0, 1.0, 9.0, 0.0, 2.0],
                &[1.7, 0.7 + 1.0],
            ),
        ];
        for (name, from, to, input, want) in CASES {
            let mut out = Vec::new();
            remix(input, *from, *to, &mut out);
            assert_eq!(out.len(), want.len(), "{name}");
            for (got, want) in out.iter().zip(*want) {
                assert!(
                    (got - want).abs() < 1e-6,
                    "{name}: {out:?} against {want:?}"
                );
            }
        }
    }

    #[test]
    fn resampling_in_pieces_equals_resampling_whole() {
        let ramp: Vec<f32> = (0..2000).map(|i| i as f32).collect();
        let mut whole = Vec::new();
        Resampler::new(44_100, 48_000, 1).process(&ramp, &mut whole);
        let mut pieces = Vec::new();
        let mut resampler = Resampler::new(44_100, 48_000, 1);
        for piece in ramp.chunks(333) {
            resampler.process(piece, &mut pieces);
        }
        assert!(
            whole.len().abs_diff(pieces.len()) <= 1,
            "{} against {}",
            whole.len(),
            pieces.len()
        );
        for (a, b) in whole.iter().zip(&pieces) {
            assert!((a - b).abs() < 1e-3, "{a} against {b}");
        }
        // A ramp stays a ramp: output frame k sits at k * 44100 / 48000.
        let k = 1000;
        assert!(
            (whole[k] - k as f32 * 44_100.0 / 48_000.0).abs() < 1e-2,
            "{}",
            whole[k]
        );
    }

    #[test]
    fn the_length_of_resampled_sound_follows_the_rates() {
        let input = vec![0.0_f32; 44_100];
        let mut out = Vec::new();
        Resampler::new(44_100, 48_000, 1).process(&input, &mut out);
        assert!(out.len().abs_diff(48_000) <= 2, "{}", out.len());
    }
}
