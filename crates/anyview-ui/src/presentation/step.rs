//! Presentation's transitions. An illegal one is an input the state lists by name and ignores.

use super::model::{
    ContentClass, Presentation, PresentationIn, PresentationOut, PresentationParams,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

type Step = (Presentation, Vec<PresentationOut>);

impl Machine for Presentation {
    type In = PresentationIn;
    type Out = PresentationOut;
    type Params = PresentationParams;

    fn step(self, input: PresentationIn, _at: Stamp, params: &PresentationParams) -> Step {
        match self {
            Presentation::Window => window(input, params),
            Presentation::Peek => peek(input),
            Presentation::Mini => mini(input),
            Presentation::Background => background(input),
        }
    }

    fn wake(&self) -> Option<Stamp> {
        match self {
            Presentation::Window
            | Presentation::Peek
            | Presentation::Mini
            | Presentation::Background => None,
        }
    }
}

fn becoming(presentation: Presentation) -> Step {
    (presentation, vec![PresentationOut::Become(presentation)])
}

fn window(input: PresentationIn, params: &PresentationParams) -> Step {
    match (input, params.content) {
        (PresentationIn::ToMini, ContentClass::Media) => becoming(Presentation::Mini),
        (PresentationIn::ToMini, ContentClass::Document)
        | (PresentationIn::ToWindow | PresentationIn::Elapsed, ContentClass::Media)
        | (PresentationIn::ToWindow | PresentationIn::Elapsed, ContentClass::Document) => {
            (Presentation::Window, vec![])
        }
    }
}

fn peek(input: PresentationIn) -> Step {
    match input {
        PresentationIn::ToWindow => becoming(Presentation::Window),
        PresentationIn::ToMini | PresentationIn::Elapsed => (Presentation::Peek, vec![]),
    }
}

fn mini(input: PresentationIn) -> Step {
    match input {
        PresentationIn::ToWindow => becoming(Presentation::Window),
        PresentationIn::ToMini | PresentationIn::Elapsed => (Presentation::Mini, vec![]),
    }
}

fn background(input: PresentationIn) -> Step {
    match input {
        PresentationIn::ToWindow => becoming(Presentation::Window),
        PresentationIn::ToMini | PresentationIn::Elapsed => (Presentation::Background, vec![]),
    }
}
