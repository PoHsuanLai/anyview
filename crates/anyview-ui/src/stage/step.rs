//! The stage region's transitions: each stage steps on its own family's inputs and the clock.

use super::media::MediaStage;
use super::model::{Stage, StageIn, StageOut, StageParams};
use super::pdf::PdfStage;
use super::raster::RasterStage;
use super::text::TextStage;
use ds_core::machine::{Elapsed, Machine};
use ds_core::time::stamp::Stamp;

type Step = (Stage, Vec<StageOut>);

/// A stage stepped on `input`, lifted into the region's types.
fn lifted<M: Machine>(
    machine: M,
    input: M::In,
    at: Stamp,
    params: &M::Params,
    wrap: fn(M) -> Stage,
    out: fn(M::Out) -> StageOut,
) -> Step {
    let (next, outs) = machine.step(input, at, params);
    (wrap(next), outs.into_iter().map(out).collect())
}

impl Machine for Stage {
    type In = StageIn;
    type Out = StageOut;
    type Params = StageParams;

    fn step(self, input: StageIn, at: Stamp, params: &StageParams) -> Step {
        match self {
            Stage::NoStage => (Stage::NoStage, vec![]),
            Stage::Raster(stage) => raster(stage, input, at, params),
            Stage::Pdf(stage) => pdf(stage, input, at, params),
            Stage::Media(stage) => media(stage, input, at, params),
            Stage::Text(stage) => text(stage, input, at, params),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(stage) => stage.wake(),
            Stage::Pdf(stage) => stage.wake(),
            Stage::Media(stage) => stage.wake(),
            Stage::Text(stage) => stage.wake(),
        }
    }
}

fn raster(stage: RasterStage, input: StageIn, at: Stamp, params: &StageParams) -> Step {
    let wrap = Stage::Raster;
    let out = StageOut::Raster;
    match input {
        StageIn::Raster(input) => lifted(stage, input, at, &params.raster, wrap, out),
        StageIn::Elapsed => lifted(stage, Elapsed.into(), at, &params.raster, wrap, out),
        StageIn::Pdf(_) | StageIn::Media(_) | StageIn::Text(_) => (Stage::Raster(stage), vec![]),
    }
}

fn pdf(stage: PdfStage, input: StageIn, at: Stamp, params: &StageParams) -> Step {
    let wrap = Stage::Pdf;
    let out = StageOut::Pdf;
    match input {
        StageIn::Pdf(input) => lifted(stage, input, at, &params.pdf, wrap, out),
        StageIn::Elapsed => lifted(stage, Elapsed.into(), at, &params.pdf, wrap, out),
        StageIn::Raster(_) | StageIn::Media(_) | StageIn::Text(_) => (Stage::Pdf(stage), vec![]),
    }
}

fn media(stage: MediaStage, input: StageIn, at: Stamp, params: &StageParams) -> Step {
    let wrap = Stage::Media;
    let out = StageOut::Media;
    match input {
        StageIn::Media(input) => lifted(stage, input, at, &params.media, wrap, out),
        StageIn::Elapsed => lifted(stage, Elapsed.into(), at, &params.media, wrap, out),
        StageIn::Raster(_) | StageIn::Pdf(_) | StageIn::Text(_) => (Stage::Media(stage), vec![]),
    }
}

fn text(stage: TextStage, input: StageIn, at: Stamp, params: &StageParams) -> Step {
    let wrap = Stage::Text;
    let out = StageOut::Text;
    match input {
        StageIn::Text(input) => lifted(stage, input, at, &params.text, wrap, out),
        StageIn::Elapsed => lifted(stage, Elapsed.into(), at, &params.text, wrap, out),
        StageIn::Raster(_) | StageIn::Pdf(_) | StageIn::Media(_) => (Stage::Text(stage), vec![]),
    }
}
