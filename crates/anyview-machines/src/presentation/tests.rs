use super::*;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

use ContentClass::{Document, Media};
use Presentation::{Background, Mini, Peek, Window};
use PresentationIn::{ToMini, ToWindow};

/// Name, content, state before, input, state after, outputs.
type Case = (
    &'static str,
    ContentClass,
    Presentation,
    PresentationIn,
    Presentation,
    &'static [PresentationOut],
);

const CASES: &[Case] = &[
    (
        "a peek is promoted to a window",
        Document,
        Peek,
        ToWindow,
        Window,
        &[PresentationOut::Become(Window)],
    ),
    (
        "a peek of media is promoted too",
        Media,
        Peek,
        ToWindow,
        Window,
        &[PresentationOut::Become(Window)],
    ),
    (
        "a peek cannot go mini before it is promoted",
        Media,
        Peek,
        ToMini,
        Peek,
        &[],
    ),
    (
        "a media window shrinks to mini",
        Media,
        Window,
        ToMini,
        Mini,
        &[PresentationOut::Become(Mini)],
    ),
    (
        "a document window cannot shrink to mini",
        Document,
        Window,
        ToMini,
        Window,
        &[],
    ),
    (
        "a window asked for a window stays",
        Media,
        Window,
        ToWindow,
        Window,
        &[],
    ),
    (
        "the mini window expands to a window",
        Media,
        Mini,
        ToWindow,
        Window,
        &[PresentationOut::Become(Window)],
    ),
    ("mini asked for mini stays", Media, Mini, ToMini, Mini, &[]),
    (
        "opening a background session makes its window",
        Media,
        Background,
        ToWindow,
        Window,
        &[PresentationOut::Become(Window)],
    ),
    (
        "a background session has no window to shrink",
        Media,
        Background,
        ToMini,
        Background,
        &[],
    ),
    (
        "the clock changes no presentation",
        Media,
        Mini,
        PresentationIn::Elapsed,
        Mini,
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, content, from, input, state, outs) in CASES {
        let params = PresentationParams { content: *content };
        let (next, out) = from.step(*input, Stamp(0), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn no_input_ever_reaches_peek_or_background() {
    // Peek and Background are how a viewer is launched; nothing transitions into them.
    for from in [Window, Peek, Mini, Background] {
        for content in [Media, Document] {
            for input in [ToWindow, ToMini, PresentationIn::Elapsed] {
                let (next, _) = from.step(input, Stamp(0), &PresentationParams { content }, &());
                assert!(
                    next == from || (next != Peek && next != Background),
                    "{from:?} on {input:?} became {next:?}"
                );
            }
        }
    }
}
