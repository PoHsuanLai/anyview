use super::*;
use crate::command::StageCommand;
use crate::typed::TypedText;
use anyview_core::{LineIndex, MediaLength, MediaTime, Zoom};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::ShortcutKey;

const MEDIA: MediaStage = MediaStage::Opening;

/// Name, state before, input, state after, outputs.
type Case = (&'static str, Stage, StageIn, Stage, &'static [StageOut]);

const CASES: &[Case] = &[
    (
        "no stage ignores every input",
        Stage::NoStage,
        StageIn::Text(TextIn::ToggleWrap),
        Stage::NoStage,
        &[],
    ),
    (
        "the media stage steps on a media input and its outputs are lifted",
        Stage::Media(MEDIA),
        StageIn::Media(MediaIn::Player(PlayerEvent::Loaded {
            length: MediaLength(MediaTime::from_secs(10)),
        })),
        Stage::Media(MediaStage::Playing {
            at: MediaTime::from_secs(0),
            length: MediaLength(MediaTime::from_secs(10)),
        }),
        &[],
    ),
    (
        "the media stage lifts its volume output",
        Stage::Media(MEDIA),
        StageIn::Media(MediaIn::Player(PlayerEvent::TracksChanged)),
        Stage::Media(MEDIA),
        &[StageOut::Media(MediaOut::TracksChanged)],
    ),
    (
        "a result for another family's stage is ignored",
        Stage::Media(MEDIA),
        StageIn::Pdf(PdfIn::NextPage),
        Stage::Media(MEDIA),
        &[],
    ),
    (
        "the clock reaches the stage that is showing",
        Stage::Text(TextStage::Reading {
            place: TextPlace {
                line: LineIndex(3),
                wrap: Wrap::On,
                view: TextView::Source,
            },
        }),
        StageIn::Elapsed,
        Stage::Text(TextStage::Reading {
            place: TextPlace {
                line: LineIndex(3),
                wrap: Wrap::On,
                view: TextView::Source,
            },
        }),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    let params = StageParams::default();
    for (name, from, input, state, outs) in CASES {
        let (next, out) = from.clone().step(input.clone(), Stamp(0), &params);
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

#[test]
fn a_family_gets_its_own_stage() {
    // name, family, stage
    let cases = [
        (
            "raster",
            StageFamily::Raster,
            Stage::Raster(RasterStage::default()),
        ),
        ("pdf", StageFamily::Pdf, Stage::Pdf(PdfStage::default())),
        (
            "media",
            StageFamily::Media,
            Stage::Media(MediaStage::Opening),
        ),
        ("peek only", StageFamily::PeekOnly, Stage::NoStage),
    ];
    for (name, family, want) in cases {
        assert_eq!(
            Stage::for_family(family, TextViews::default()),
            want,
            "{name}"
        );
    }
    assert_eq!(
        Stage::for_family(StageFamily::Text, TextViews::SourceOnly),
        Stage::Text(TextStage::opened(TextViews::SourceOnly))
    );
}

#[test]
fn a_command_means_what_the_showing_stage_makes_of_it() {
    let params = StageParams::default();
    let raster = Stage::Raster(RasterStage::default());
    let pdf = Stage::Pdf(PdfStage::default());
    let text = Stage::Text(TextStage::default());
    let media = Stage::Media(MediaStage::default());
    // name, stage, command, input
    let cases = [
        (
            "fit on an image",
            &raster,
            StageCommand::ZoomToFit,
            Some(StageIn::Raster(RasterIn::SetZoom {
                zoom: Zoom::Fit,
                at: params.raster.centre,
            })),
        ),
        (
            "fit on a pdf",
            &pdf,
            StageCommand::ZoomToFit,
            Some(StageIn::Pdf(PdfIn::SetZoom(Zoom::Fit))),
        ),
        (
            "find on a pdf opens an empty bar",
            &pdf,
            StageCommand::Find,
            Some(StageIn::Pdf(PdfIn::Find(TypedText::EMPTY))),
        ),
        (
            "source on text",
            &text,
            StageCommand::ToggleSource,
            Some(StageIn::Text(TextIn::ToggleSource)),
        ),
        (
            "source on a pdf means nothing",
            &pdf,
            StageCommand::ToggleSource,
            None,
        ),
        (
            "play on media",
            &media,
            StageCommand::TogglePlayback,
            Some(StageIn::Media(MediaIn::Toggle)),
        ),
        (
            "zoom on media means nothing",
            &media,
            StageCommand::ZoomIn,
            None,
        ),
        (
            "anything on no stage means nothing",
            &Stage::NoStage,
            StageCommand::Find,
            None,
        ),
    ];
    for (name, stage, command, want) in cases {
        assert_eq!(stage.input_for(command, &params), want, "{name}");
    }
}

#[test]
fn escape_undoes_what_the_stage_has_open() {
    let finding = Stage::Pdf(PdfStage::Finding {
        query: TypedText::from_static("cat"),
        hits: FindHits::Pending,
        view: PageView {
            page: anyview_core::PageIndex(0),
            offset: anyview_core::Permille(0),
            zoom: Zoom::Fit,
        },
    });
    assert_eq!(finding.dismissal(), Some(StageIn::Pdf(PdfIn::CloseFind)));
    assert_eq!(Stage::Pdf(PdfStage::default()).dismissal(), None);
    assert_eq!(Stage::NoStage.dismissal(), None);
}

#[test]
fn keys_stand_for_commands() {
    use ShortcutKey::{Char, Down, End, Home, Left, PageDown, Right, Shift, Space, Super, Up};
    // name, keys, command
    let cases: [(&str, &[ShortcutKey], Option<StageCommand>); 15] = [
        ("plus", &[Char('+')], Some(StageCommand::ZoomIn)),
        ("equals", &[Char('=')], Some(StageCommand::ZoomIn)),
        ("minus", &[Char('-')], Some(StageCommand::ZoomOut)),
        ("zero", &[Char('0')], Some(StageCommand::ZoomToFit)),
        ("one", &[Char('1')], Some(StageCommand::ZoomToActual)),
        ("find", &[Super, Char('f')], Some(StageCommand::Find)),
        (
            "find previous",
            &[Shift, Super, Char('g')],
            Some(StageCommand::FindPrevious),
        ),
        ("space", &[Space], Some(StageCommand::TogglePlayback)),
        (
            "shift left seeks, bare left does not",
            &[Shift, Left],
            Some(StageCommand::SeekBack),
        ),
        ("page down", &[PageDown], Some(StageCommand::NextPage)),
        ("bare right walks the sequence instead", &[Right], None),
        (
            "home scrolls to the start of a text",
            &[Home],
            Some(StageCommand::ScrollToStart),
        ),
        (
            "end scrolls to the end",
            &[End],
            Some(StageCommand::ScrollToEnd),
        ),
        ("up is a line up", &[Up], Some(StageCommand::LineUp)),
        ("down is a line down", &[Down], Some(StageCommand::LineDown)),
    ];
    for (name, keys, want) in cases {
        assert_eq!(StageCommand::from_key(keys), want, "{name}");
    }
}

#[test]
fn only_a_stage_that_has_a_place_or_a_search_says_so_in_its_outputs() {
    use crate::stage::{FindOut, HitIndex};
    let resume = anyview_core::Resume::Text { line: LineIndex(4) };
    // name, output, the place it remembers, the search it asks for
    let cases: Vec<(
        &str,
        StageOut,
        Option<anyview_core::Resume>,
        Option<FindOut>,
    )> = vec![
        (
            "a text remembers its line",
            StageOut::Text(TextOut::Remember(resume.clone())),
            Some(resume.clone()),
            None,
        ),
        (
            "a text searches",
            StageOut::Text(TextOut::Find(FindOut::ShowHit(HitIndex(2)))),
            None,
            Some(FindOut::ShowHit(HitIndex(2))),
        ),
        (
            "a text scrolling is neither",
            StageOut::Text(TextOut::ScrollTo(LineIndex(4))),
            None,
            None,
        ),
        (
            "a picture remembers its zoom",
            StageOut::Raster(RasterOut::Remember(anyview_core::Resume::Nothing)),
            Some(anyview_core::Resume::Nothing),
            None,
        ),
        (
            "a recording asks for neither",
            StageOut::Media(MediaOut::TracksChanged),
            None,
            None,
        ),
    ];
    for (name, out, remembered, find) in cases {
        assert_eq!(out.remembered(), remembered.as_ref(), "{name}: place");
        assert_eq!(out.find(), find.as_ref(), "{name}: search");
    }
}

#[test]
fn a_find_holds_the_keys_only_while_it_is_up() {
    let finding = Stage::Text(TextStage::Finding {
        query: TypedText::EMPTY,
        hits: FindHits::Idle,
        place: TextPlace {
            line: LineIndex(0),
            wrap: Wrap::On,
            view: TextView::Source,
        },
    });
    assert!(finding.is_finding());
    assert!(!Stage::default().is_finding());
    assert!(!Stage::Text(TextStage::default()).is_finding());
}

#[test]
fn the_line_and_edge_keys_scroll_a_pdf_as_they_scroll_a_text() {
    use super::pdf::PdfIn;
    use anyview_core::{PageCount, PageIndex, Permille, Zoom};
    let params = StageParams {
        pdf: super::pdf::PdfParams {
            pages: PageCount::new(5).unwrap(),
            ..super::pdf::PdfParams::default()
        },
        ..StageParams::default()
    };
    let stage = Stage::Pdf(super::pdf::PdfStage::Reading {
        view: super::pdf::PageView {
            page: PageIndex(1),
            offset: Permille(500),
            zoom: Zoom::Fit,
        },
    });
    let to = |page, offset| {
        Some(StageIn::Pdf(PdfIn::GoTo(super::pdf::Destination {
            page: PageIndex(page),
            offset: Permille(offset),
        })))
    };
    // name, command, input
    let cases = [
        ("a line down", StageCommand::LineDown, to(1, 580)),
        ("a line up", StageCommand::LineUp, to(1, 420)),
        ("home", StageCommand::ScrollToStart, to(0, 0)),
        ("end", StageCommand::ScrollToEnd, to(4, 0)),
    ];
    for (name, command, want) in cases {
        assert_eq!(stage.input_for(command, &params), want, "{name}");
    }
}
