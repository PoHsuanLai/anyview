//! A queue of audio samples, so an encoder that takes frames of one fixed size is fed them however
//! the resampler happens to cut its output.

use ff::ChannelLayout;
use ff::format::Sample;
use ff::frame::Audio;
use ffmpeg_next as ff;

/// Samples waiting to be cut into frames of a size.
pub(super) struct SampleFifo {
    format: Sample,
    layout: ChannelLayout,
    rate: u32,
    planes: Vec<Vec<u8>>,
    /// The bytes of one sample in one plane: a channel's for planar audio, all channels' for
    /// packed.
    stride: usize,
    samples: usize,
}

impl SampleFifo {
    /// An empty queue of audio in `format`, `layout` and `rate`.
    pub(super) fn new(format: Sample, layout: ChannelLayout, rate: u32) -> SampleFifo {
        let channels = usize::try_from(layout.channels()).unwrap_or(1).max(1);
        let (planes, stride) = if format.is_planar() {
            (channels, format.bytes())
        } else {
            (1, format.bytes() * channels)
        };
        SampleFifo {
            format,
            layout,
            rate,
            planes: vec![Vec::new(); planes],
            stride,
            samples: 0,
        }
    }

    /// How many samples are waiting.
    pub(super) fn len(&self) -> usize {
        self.samples
    }

    /// Queue the samples of `frame`, which must be in the queue's format.
    pub(super) fn push(&mut self, frame: &Audio) {
        let bytes = frame.samples() * self.stride;
        for (index, plane) in self.planes.iter_mut().enumerate() {
            plane.extend_from_slice(&frame.data(index)[..bytes]);
        }
        self.samples += frame.samples();
    }

    /// A frame of the oldest `count` samples, or of all there are when fewer.
    pub(super) fn pop(&mut self, count: usize) -> Audio {
        let taken = count.min(self.samples);
        let bytes = taken * self.stride;
        let mut frame = Audio::new(self.format, taken, self.layout);
        frame.set_rate(self.rate);
        for (index, plane) in self.planes.iter_mut().enumerate() {
            frame.data_mut(index)[..bytes].copy_from_slice(&plane[..bytes]);
            plane.drain(..bytes);
        }
        self.samples -= taken;
        frame
    }
}
