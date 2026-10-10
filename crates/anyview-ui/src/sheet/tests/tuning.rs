//! The export sheet's options: each one changes the draft it belongs to, is dropped by a draft
//! it means nothing to, and is brought inside its limits.

use crate::sheet::*;
use anyview_core::{
    AudioTarget, Bitrate, Dpi, ExportChoice, MediaExport, MediaExportKind, MediaTime,
    MetadataCarry, Orientation, PageCount, PageIndex, PageRange, PageSelection, PaperSize,
    PdfExport, PdfExportKind, Percent, Permille, PixelLen, PixelSize, PrintLayout, Quality,
    RasterExport, RasterExportKind, RasterTarget, Resize, TextExport, TimeRange,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

fn marks() -> TimeRange {
    TimeRange::new(
        MediaTime::from_millis(1_000),
        Some(MediaTime::from_millis(5_000)),
    )
    .unwrap()
}

fn facts() -> ExportFacts {
    ExportFacts {
        pages: PageCount::new(10),
        page: PageIndex(3),
        image: Some(PixelSize {
            width: PixelLen(4000),
            height: PixelLen(3000),
        }),
        marks: Some(marks()),
    }
}

fn params(facts: ExportFacts) -> SheetParams {
    SheetParams {
        export: facts,
        ..SheetParams::default()
    }
}

/// The sheet after `inputs`, opened on `draft`.
fn after(draft: ExportDraft, facts: ExportFacts, inputs: &[SheetIn]) -> Sheet {
    let params = params(facts);
    let (mut sheet, _) = Sheet::Closed.step(SheetIn::OpenExport(draft), Stamp(0), &params, &());
    for input in inputs {
        sheet = sheet.step(input.clone(), Stamp(0), &params, &()).0;
    }
    sheet
}

fn drafted(sheet: &Sheet) -> ExportDraft {
    match sheet {
        Sheet::Export { draft, .. } => *draft,
        Sheet::Closed
        | Sheet::Unavailable { .. }
        | Sheet::ConfirmTrash
        | Sheet::ConfirmEdit { .. }
        | Sheet::Rename { .. }
        | Sheet::SaveCopy { .. }
        | Sheet::Revert { .. }
        | Sheet::NoVersions
        | Sheet::Unsaved(_)
        | Sheet::ConfirmReplace
        | Sheet::Helper { .. }
        | Sheet::Picture(_) => panic!("the export sheet is up: {sheet:?}"),
    }
}

fn tuned(draft: ExportDraft, option: ExportOption) -> ExportDraft {
    drafted(&after(draft, facts(), &[SheetIn::Tune(option)]))
}

fn jpeg(percent: u16) -> RasterTarget {
    RasterTarget::Jpeg(Quality::clamped(Percent(percent)))
}

fn image(target: RasterTarget) -> ExportDraft {
    ExportDraft::Raster(RasterExport::Image(
        target,
        Resize::Original,
        MetadataCarry::default(),
    ))
}

fn kind(kind: RasterExportKind) -> ExportDraft {
    ExportDraft::Raster(RasterExport::default_for(kind))
}

#[test]
fn quality_changes_a_jpeg_and_an_avif_and_is_clamped() {
    // name, draft, the percent asked, the draft after
    let cases = [
        ("jpeg", image(jpeg(90)), 40, image(jpeg(40))),
        (
            "avif",
            kind(RasterExportKind::Avif),
            30,
            image(RasterTarget::Avif(Quality::clamped(Percent(30)))),
        ),
        ("above the top", image(jpeg(90)), 400, image(jpeg(100))),
        ("below the bottom", image(jpeg(90)), 0, image(jpeg(1))),
        (
            "png has none",
            image(RasterTarget::Png),
            40,
            image(RasterTarget::Png),
        ),
        (
            "webp has none",
            kind(RasterExportKind::Webp),
            40,
            kind(RasterExportKind::Webp),
        ),
    ];
    for (name, draft, percent, want) in cases {
        assert_eq!(
            tuned(draft, ExportOption::Quality(Percent(percent))),
            want,
            "{name}"
        );
    }
}

#[test]
fn the_size_of_a_picture_is_kept_inside_what_can_be_made() {
    let sized = |resize| {
        ExportDraft::Raster(RasterExport::Image(
            RasterTarget::Png,
            resize,
            MetadataCarry::Keep,
        ))
    };
    let cases = [
        (
            "half",
            Resize::Scaled(Permille(500)),
            Resize::Scaled(Permille(500)),
        ),
        (
            "a quarter",
            Resize::Scaled(Permille(250)),
            Resize::Scaled(Permille(250)),
        ),
        ("original again", Resize::Original, Resize::Original),
        (
            "a long edge",
            Resize::LongEdge(PixelLen(800)),
            Resize::LongEdge(PixelLen(800)),
        ),
        (
            "a long edge of nothing",
            Resize::LongEdge(PixelLen(0)),
            Resize::LongEdge(PixelLen(1)),
        ),
        (
            "a long edge past the limit",
            Resize::LongEdge(PixelLen(u32::MAX)),
            Resize::LongEdge(MAX_LONG_EDGE),
        ),
        (
            "a scale of nothing",
            Resize::Scaled(Permille(0)),
            Resize::Scaled(Permille(1)),
        ),
        (
            "a scale past whole",
            Resize::Scaled(Permille(5000)),
            Resize::Scaled(Permille(1000)),
        ),
    ];
    for (name, asked, want) in cases {
        let got = tuned(sized(Resize::Original), ExportOption::Resize(asked));
        assert_eq!(got, sized(want), "{name}");
    }
}

#[test]
fn metadata_is_all_but_the_location_unless_the_person_says_otherwise() {
    let with = |keep| {
        ExportDraft::Raster(RasterExport::Image(
            RasterTarget::Png,
            Resize::Original,
            keep,
        ))
    };
    assert_eq!(
        image(RasterTarget::Png),
        with(MetadataCarry::StripLocation),
        "a new export does not say where the photo was taken"
    );
    assert_eq!(
        kind(RasterExportKind::Png),
        with(MetadataCarry::StripLocation),
        "nor does the one a format starts with"
    );
    for keep in [
        MetadataCarry::Keep,
        MetadataCarry::StripLocation,
        MetadataCarry::Drop,
    ] {
        assert_eq!(
            tuned(image(RasterTarget::Png), ExportOption::Metadata(keep)),
            with(keep),
            "{keep:?}"
        );
    }
    let pdf = kind(RasterExportKind::Pdf);
    assert_eq!(tuned(pdf, ExportOption::Metadata(MetadataCarry::Drop)), pdf);
}

#[test]
fn a_format_picked_after_another_keeps_the_size_the_metadata_and_the_quality() {
    let before = ExportDraft::Raster(RasterExport::Image(
        jpeg(70),
        Resize::Scaled(Permille(500)),
        MetadataCarry::Drop,
    ));
    let after = |kind| {
        drafted(&after(
            before,
            facts(),
            &[SheetIn::PickKind(ExportKindPick::Raster(kind))],
        ))
    };
    let raster = |target| {
        ExportDraft::Raster(RasterExport::Image(
            target,
            Resize::Scaled(Permille(500)),
            MetadataCarry::Drop,
        ))
    };
    assert_eq!(
        after(RasterExportKind::Avif),
        raster(RasterTarget::Avif(Quality::clamped(Percent(70)))),
        "JPEG to AVIF"
    );
    assert_eq!(after(RasterExportKind::Png), raster(RasterTarget::Png));
    assert_eq!(
        after(RasterExportKind::Pdf),
        ExportDraft::Raster(RasterExport::Pdf)
    );
    // From a format with no quality, the next starts on its own.
    assert_eq!(
        drafted(&self::after(
            raster(RasterTarget::Png),
            facts(),
            &[SheetIn::PickKind(ExportKindPick::Raster(
                RasterExportKind::Jpeg
            ))]
        )),
        raster(RasterTarget::default_jpeg())
    );
}

#[test]
fn page_images_keep_the_quality_when_the_format_changes_between_lossy_ones() {
    let sheet = after(
        images(),
        facts(),
        &[
            SheetIn::Tune(ExportOption::ImageFormat(RasterExportKind::Jpeg)),
            SheetIn::Tune(ExportOption::Quality(Percent(55))),
            SheetIn::Tune(ExportOption::ImageFormat(RasterExportKind::Avif)),
        ],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Pdf(PdfExport::PageImages(
            PageSelection::All,
            RasterTarget::Avif(Quality::clamped(Percent(55))),
            Dpi::SCREEN
        ))
    );
}

#[test]
fn a_frame_keeps_its_quality_between_lossy_formats() {
    let sheet = after(
        media(MediaExportKind::FrameJpeg),
        facts(),
        &[
            SheetIn::Tune(ExportOption::Quality(Percent(40))),
            SheetIn::PickKind(ExportKindPick::Media(MediaExportKind::FrameAvif)),
        ],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Media(MediaExport::CurrentFrame(RasterTarget::Avif(
            Quality::clamped(Percent(40))
        )))
    );
}

#[test]
fn a_size_lights_a_segment_only_when_it_is_one_of_them() {
    let cases = [
        (Resize::Original, Some(SizePick::Original)),
        (Resize::Scaled(Permille(500)), Some(SizePick::Half)),
        (Resize::Scaled(Permille(250)), Some(SizePick::Quarter)),
        (Resize::Scaled(Permille(300)), None),
        (Resize::Scaled(Permille(1000)), None),
        (Resize::LongEdge(PixelLen(800)), Some(SizePick::LongEdge)),
    ];
    for (resize, want) in cases {
        assert_eq!(SizePick::of(resize), want, "{resize:?}");
    }
    for pick in [
        SizePick::Original,
        SizePick::Half,
        SizePick::Quarter,
        SizePick::LongEdge,
    ] {
        assert_eq!(SizePick::of(pick.resize(PixelLen(800))), Some(pick));
    }
}

fn pdf_pages() -> ExportDraft {
    ExportDraft::Pdf(PdfExport::default_for(PdfExportKind::Pdf))
}

fn images() -> ExportDraft {
    ExportDraft::Pdf(PdfExport::default_for(PdfExportKind::PageImages))
}

fn range(first: u32, last: u32) -> PageSelection {
    PageSelection::Range(PageRange::new(PageIndex(first), PageIndex(last)).unwrap())
}

fn pages_of(draft: ExportDraft) -> PageSelection {
    match draft {
        ExportDraft::Pdf(PdfExport::Pdf(pages) | PdfExport::PageImages(pages, ..)) => pages,
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(PdfExport::PlainText | PdfExport::Markdown)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => panic!("a draft with pages: {draft:?}"),
    }
}

#[test]
fn pages_are_all_the_current_one_or_a_range_inside_the_document() {
    for (name, draft) in [("pdf", pdf_pages()), ("page images", images())] {
        let go = |inputs: &[ExportOption]| {
            let inputs: Vec<SheetIn> = inputs.iter().copied().map(SheetIn::Tune).collect();
            after(draft, facts(), &inputs)
        };
        let sheet = go(&[ExportOption::Pages(PageSpan::Current)]);
        assert_eq!(
            pages_of(drafted(&sheet)),
            range(3, 3),
            "{name}: the page the reader is on"
        );
        let sheet = go(&[ExportOption::Pages(PageSpan::Between)]);
        assert_eq!(
            pages_of(drafted(&sheet)),
            range(0, 9),
            "{name}: a range starts as the whole"
        );
        let sheet = go(&[
            ExportOption::Pages(PageSpan::Between),
            ExportOption::PagesFrom(4),
            ExportOption::PagesTo(6),
        ]);
        assert_eq!(
            pages_of(drafted(&sheet)),
            range(3, 5),
            "{name}: pages 4 to 6"
        );
        let sheet = go(&[
            ExportOption::Pages(PageSpan::Between),
            ExportOption::PagesFrom(4),
            ExportOption::PagesTo(6),
            ExportOption::Pages(PageSpan::All),
        ]);
        assert_eq!(
            pages_of(drafted(&sheet)),
            PageSelection::All,
            "{name}: back to all"
        );
    }
}

#[test]
fn a_page_range_never_leaves_the_document_or_runs_backwards() {
    let range_after = |options: &[ExportOption]| {
        let mut inputs = vec![SheetIn::Tune(ExportOption::Pages(PageSpan::Between))];
        inputs.extend(options.iter().copied().map(SheetIn::Tune));
        pages_of(drafted(&after(pdf_pages(), facts(), &inputs)))
    };
    // name, the options after choosing a range, the range left
    let cases = [
        (
            "first past the end",
            vec![ExportOption::PagesFrom(99)],
            range(9, 9),
        ),
        (
            "last past the end",
            vec![ExportOption::PagesTo(99)],
            range(0, 9),
        ),
        (
            "first of zero",
            vec![ExportOption::PagesFrom(0)],
            range(0, 9),
        ),
        (
            "first past the last drags the last along",
            vec![ExportOption::PagesTo(3), ExportOption::PagesFrom(8)],
            range(7, 7),
        ),
        (
            "last before the first drags the first along",
            vec![ExportOption::PagesFrom(8), ExportOption::PagesTo(2)],
            range(1, 1),
        ),
    ];
    for (name, options, want) in cases {
        assert_eq!(range_after(&options), want, "{name}");
    }
}

#[test]
fn a_one_page_document_has_no_page_choice_and_a_range_without_the_choice_is_ignored() {
    let one = ExportFacts {
        pages: PageCount::new(1),
        ..facts()
    };
    let sheet = after(
        pdf_pages(),
        one,
        &[SheetIn::Tune(ExportOption::Pages(PageSpan::Current))],
    );
    assert_eq!(drafted(&sheet), pdf_pages());
    assert!(pdf_pages().controls(&one, PageSpan::All).is_empty());
    // Moving an end while the choice is not a range does nothing.
    let sheet = after(
        pdf_pages(),
        facts(),
        &[SheetIn::Tune(ExportOption::PagesFrom(3))],
    );
    assert_eq!(drafted(&sheet), pdf_pages());
    // A document whose length is not known offers none either.
    let unknown = ExportFacts {
        pages: None,
        ..facts()
    };
    assert!(pdf_pages().controls(&unknown, PageSpan::All).is_empty());
}

#[test]
fn page_images_take_a_format_a_quality_and_a_resolution() {
    let ExportDraft::Pdf(PdfExport::PageImages(_, target, dpi)) = images() else {
        panic!("page images")
    };
    assert_eq!((target, dpi), (RasterTarget::Png, Dpi::SCREEN));
    let sheet = after(
        images(),
        facts(),
        &[
            SheetIn::Tune(ExportOption::ImageFormat(RasterExportKind::Jpeg)),
            SheetIn::Tune(ExportOption::Quality(Percent(55))),
            SheetIn::Tune(ExportOption::Resolution(Dpi::PRINT)),
        ],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Pdf(PdfExport::PageImages(
            PageSelection::All,
            jpeg(55),
            Dpi::PRINT
        ))
    );
    // The format that is not an image is refused, and the resolution means nothing to a PDF.
    let kept = tuned(images(), ExportOption::ImageFormat(RasterExportKind::Pdf));
    assert_eq!(kept, images());
    assert_eq!(
        tuned(pdf_pages(), ExportOption::Resolution(Dpi::PRINT)),
        pdf_pages()
    );
    for dpi in [Dpi::PAGE, Dpi::SCREEN, Dpi::PRINT] {
        let got = tuned(images(), ExportOption::Resolution(dpi));
        assert_eq!(
            got,
            ExportDraft::Pdf(PdfExport::PageImages(
                PageSelection::All,
                RasterTarget::Png,
                dpi
            ))
        );
    }
}

#[test]
fn paper_and_orientation_set_the_printed_page_of_a_text() {
    let pdf = ExportDraft::Text(TextExport::default_for(anyview_core::TextExportKind::Pdf));
    let sheet = after(
        pdf,
        facts(),
        &[
            SheetIn::Tune(ExportOption::Paper(PaperSize::Letter)),
            SheetIn::Tune(ExportOption::Orientation(Orientation::Landscape)),
        ],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Text(TextExport::Pdf(PrintLayout {
            paper: PaperSize::Letter,
            orientation: Orientation::Landscape,
        }))
    );
    let plain = ExportDraft::Text(TextExport::PlainText);
    assert_eq!(tuned(plain, ExportOption::Paper(PaperSize::A3)), plain);
    assert_eq!(
        tuned(plain, ExportOption::Orientation(Orientation::Landscape)),
        plain
    );
}

fn media(kind: MediaExportKind) -> ExportDraft {
    ExportDraft::Media(MediaExport::default_for(kind))
}

#[test]
fn a_trim_keeps_the_whole_recording_or_what_is_between_the_marks() {
    // The sheet opens a trim on the marks when there are some.
    let sheet = after(media(MediaExportKind::Trim), facts(), &[]);
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Media(MediaExport::Trim(marks()))
    );
    let whole = tuned(
        media(MediaExportKind::Trim),
        ExportOption::Trim(TrimSpan::Whole),
    );
    assert_eq!(
        whole,
        ExportDraft::Media(MediaExport::Trim(TimeRange::WHOLE))
    );
    let sheet = after(
        media(MediaExportKind::Trim),
        facts(),
        &[
            SheetIn::Tune(ExportOption::Trim(TrimSpan::Whole)),
            SheetIn::Tune(ExportOption::Trim(TrimSpan::Marks)),
        ],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Media(MediaExport::Trim(marks()))
    );
    // With no marks there is nothing to choose, and the marks cannot be chosen.
    let none = ExportFacts {
        marks: None,
        ..facts()
    };
    let sheet = after(
        media(MediaExportKind::Trim),
        none,
        &[SheetIn::Tune(ExportOption::Trim(TrimSpan::Marks))],
    );
    assert_eq!(drafted(&sheet), media(MediaExportKind::Trim));
    assert!(
        media(MediaExportKind::Trim)
            .controls(&none, PageSpan::All)
            .is_empty()
    );
    // Picking Trim from the format list seeds the marks too.
    let sheet = after(
        media(MediaExportKind::ToMp3),
        facts(),
        &[SheetIn::PickKind(ExportKindPick::Media(
            MediaExportKind::Trim,
        ))],
    );
    assert_eq!(
        drafted(&sheet),
        ExportDraft::Media(MediaExport::Trim(marks()))
    );
}

#[test]
fn bitrate_is_for_lossy_audio_only_and_is_clamped() {
    let rate = |kbps| Bitrate::from_kbps(kbps);
    for (name, kind, want) in [
        (
            "m4a",
            MediaExportKind::ToM4a,
            Some(AudioTarget::M4a(rate(256))),
        ),
        (
            "mp3",
            MediaExportKind::ToMp3,
            Some(AudioTarget::Mp3(rate(256))),
        ),
        (
            "opus",
            MediaExportKind::ToOpus,
            Some(AudioTarget::Opus(rate(256))),
        ),
        ("flac", MediaExportKind::ToFlac, None),
        ("wav", MediaExportKind::ToWav, None),
        ("copy", MediaExportKind::ExtractAudio, None),
    ] {
        let before = media(kind);
        let got = tuned(before, ExportOption::Bitrate(rate(256)));
        match want {
            Some(target) => assert_eq!(
                got,
                ExportDraft::Media(MediaExport::AudioOnly(target)),
                "{name}"
            ),
            None => assert_eq!(got, before, "{name}"),
        }
    }
    let high = tuned(
        media(MediaExportKind::ToMp3),
        ExportOption::Bitrate(rate(100_000)),
    );
    assert_eq!(
        high,
        ExportDraft::Media(MediaExport::AudioOnly(AudioTarget::Mp3(Bitrate::MAX)))
    );
}

#[test]
fn a_frame_has_a_quality_when_it_is_a_jpeg_or_an_avif() {
    let frame = tuned(
        media(MediaExportKind::FrameJpeg),
        ExportOption::Quality(Percent(70)),
    );
    assert_eq!(
        frame,
        ExportDraft::Media(MediaExport::CurrentFrame(jpeg(70)))
    );
    let png = media(MediaExportKind::FramePng);
    assert_eq!(tuned(png, ExportOption::Quality(Percent(70))), png);
}

#[test]
fn an_option_of_another_format_changes_nothing() {
    let drafts = [
        image(RasterTarget::Png),
        pdf_pages(),
        ExportDraft::Text(TextExport::PlainText),
        media(MediaExportKind::ToWav),
    ];
    let options = [
        ExportOption::Paper(PaperSize::A3),
        ExportOption::Bitrate(Bitrate::from_kbps(96)),
        ExportOption::Trim(TrimSpan::Marks),
    ];
    for draft in drafts {
        for option in options {
            assert_eq!(tuned(draft, option), draft, "{draft:?} {option:?}");
        }
    }
}

#[test]
fn choosing_another_format_starts_its_options_afresh_and_the_page_choice_over() {
    let sheet = after(
        pdf_pages(),
        facts(),
        &[
            SheetIn::Tune(ExportOption::Pages(PageSpan::Current)),
            SheetIn::PickKind(ExportKindPick::Pdf(PdfExportKind::PageImages)),
        ],
    );
    assert_eq!(drafted(&sheet), images());
    assert!(matches!(
        sheet,
        Sheet::Export {
            span: PageSpan::All,
            ..
        }
    ));
}

#[test]
fn confirming_exports_the_options_as_chosen() {
    let params = params(facts());
    let (sheet, _) =
        Sheet::Closed.step(SheetIn::OpenExport(image(jpeg(90))), Stamp(0), &params, &());
    let (sheet, _) = sheet.step(
        SheetIn::Tune(ExportOption::Quality(Percent(33))),
        Stamp(0),
        &params,
        &(),
    );
    let (sheet, _) = sheet.step(
        SheetIn::Tune(ExportOption::Metadata(MetadataCarry::Drop)),
        Stamp(0),
        &params,
        &(),
    );
    let (sheet, _) = sheet.step(
        SheetIn::Tune(ExportOption::Resize(Resize::LongEdge(PixelLen(1024)))),
        Stamp(0),
        &params,
        &(),
    );
    let (closed, outs) = sheet.step(SheetIn::Confirm, Stamp(0), &params, &());
    assert_eq!(closed, Sheet::Closed);
    assert_eq!(
        outs,
        [
            SheetOut::Export(ExportDraft::Raster(RasterExport::Image(
                jpeg(33),
                Resize::LongEdge(PixelLen(1024)),
                MetadataCarry::Drop,
            ))),
            SheetOut::Closed
        ]
    );
}

#[test]
fn each_format_shows_the_controls_that_mean_something_for_it() {
    use ExportControl::*;
    let f = facts();
    let all = PageSpan::All;
    // name, draft, page choice, controls
    let cases: Vec<(&str, ExportDraft, PageSpan, Vec<ExportControl>)> = vec![
        ("png", image(RasterTarget::Png), all, vec![Size, Metadata]),
        ("jpeg", image(jpeg(90)), all, vec![Quality, Size, Metadata]),
        (
            "avif",
            kind(RasterExportKind::Avif),
            all,
            vec![Quality, Size, Metadata],
        ),
        (
            "webp",
            kind(RasterExportKind::Webp),
            all,
            vec![Size, Metadata],
        ),
        (
            "tiff",
            kind(RasterExportKind::Tiff),
            all,
            vec![Size, Metadata],
        ),
        ("picture as pdf", kind(RasterExportKind::Pdf), all, vec![]),
        ("pdf", pdf_pages(), all, vec![Pages]),
        (
            "pdf range",
            pdf_pages(),
            PageSpan::Between,
            vec![Pages, PageRange],
        ),
        ("pdf current", pdf_pages(), PageSpan::Current, vec![Pages]),
        (
            "page images",
            images(),
            all,
            vec![Pages, ImageFormat, Resolution],
        ),
        (
            "page images range",
            images(),
            PageSpan::Between,
            vec![Pages, PageRange, ImageFormat, Resolution],
        ),
        (
            "plain text of a pdf",
            ExportDraft::Pdf(PdfExport::PlainText),
            all,
            vec![],
        ),
        (
            "markdown of a pdf",
            ExportDraft::Pdf(PdfExport::Markdown),
            all,
            vec![],
        ),
        (
            "text as pdf",
            ExportDraft::Text(TextExport::default_for(anyview_core::TextExportKind::Pdf)),
            all,
            vec![Paper, Orientation],
        ),
        (
            "plain text",
            ExportDraft::Text(TextExport::PlainText),
            all,
            vec![],
        ),
        ("frame png", media(MediaExportKind::FramePng), all, vec![]),
        (
            "frame jpeg",
            media(MediaExportKind::FrameJpeg),
            all,
            vec![Quality],
        ),
        (
            "trim with marks",
            media(MediaExportKind::Trim),
            all,
            vec![Range],
        ),
        ("m4a", media(MediaExportKind::ToM4a), all, vec![Bitrate]),
        ("mp3", media(MediaExportKind::ToMp3), all, vec![Bitrate]),
        ("opus", media(MediaExportKind::ToOpus), all, vec![Bitrate]),
        ("flac", media(MediaExportKind::ToFlac), all, vec![]),
        ("wav", media(MediaExportKind::ToWav), all, vec![]),
        (
            "audio as it is",
            media(MediaExportKind::ExtractAudio),
            all,
            vec![],
        ),
    ];
    for (name, draft, span, want) in cases {
        assert_eq!(draft.controls(&f, span), want, "{name}");
    }
}

#[test]
fn the_file_is_saved_beside_the_original_under_the_new_extension() {
    // name, draft, original, saved as
    let cases = [
        ("png to jpeg", image(jpeg(90)), "photo.png", "photo.jpg"),
        ("a dotted name", image(jpeg(90)), "a.b.png", "a.b.jpg"),
        ("no extension", image(jpeg(90)), "photo", "photo.jpg"),
        ("a hidden file", image(jpeg(90)), ".photo", ".photo.jpg"),
        (
            "pdf of text",
            ExportDraft::Text(TextExport::Pdf(PrintLayout::default())),
            "notes.md",
            "notes.pdf",
        ),
        (
            "a trim keeps its extension",
            media(MediaExportKind::Trim),
            "clip.mp4",
            "clip.mp4",
        ),
        (
            "audio to mp3",
            media(MediaExportKind::ToMp3),
            "song.flac",
            "song.mp3",
        ),
    ];
    for (name, draft, original, want) in cases {
        assert_eq!(draft.saved_as(original).as_deref(), Some(want), "{name}");
    }
}

#[test]
fn a_file_with_no_name_has_no_saved_as_note() {
    assert_eq!(image(jpeg(90)).saved_as(""), None);
}

#[test]
fn every_format_has_a_name_and_a_line_about_it() {
    for draft in [
        image(RasterTarget::Png),
        pdf_pages(),
        ExportDraft::Text(TextExport::PlainText),
        media(MediaExportKind::Trim),
    ] {
        for pick in draft.choices() {
            assert!(!kind_name(pick).is_empty(), "{pick:?} is named");
            assert!(
                kind_hint(pick).ends_with('.'),
                "{pick:?} has a line about it"
            );
        }
    }
}
