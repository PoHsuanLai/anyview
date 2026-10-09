//! The export dialog under the harness: it appears in place, lists the formats on one side and the
//! chosen format's options on the other, shows the controls that mean something for the file,
//! stands the same width at the scales a desktop runs at, and asks the host for what was chosen.

use crate::pdf_fixture;
use crate::support;

use anyview_core::{
    Dpi, MetadataCarry, PageIndex, PageRange, PageSelection, PdfExport, Percent, Permille, Quality,
    RasterExport, RasterTarget, Resize, TextExport,
};
use anyview_ui::{ExportDraft, HostRequest};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::path::PathBuf;
use support::{Requests, Wiring, settle, text_file, wired};

const SCALES: [u16; 2] = [100, 200];

const FORMATS: &str = ".ds-sheet .ds-list[aria-label=\"Format\"] .ds-list-item";

fn open(path: PathBuf, scale_percent: u16) -> (Harness, Requests) {
    let wiring = Wiring {
        viewport: Some(Viewport {
            width: 900,
            height: 600,
            scale_percent,
        }),
        ..Wiring::default()
    };
    let (mut harness, requests, _) = wired(&[path], 0, Appearance::default(), wiring);
    settle(&mut harness);
    (harness, requests)
}

fn export(harness: &mut Harness) {
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('k')));
    settle(harness);
    for letter in "export".chars() {
        harness.send(Input::key(ShortcutKey::Char(letter)));
    }
    settle(harness);
    harness.send(Input::key(ShortcutKey::Enter));
    settle(harness);
}

fn click(harness: &mut Harness, selector: &str) {
    let at = harness
        .centre(selector)
        .unwrap_or_else(|| panic!("{selector} is on screen"));
    harness.send(Input::click(at));
    settle(harness);
}

fn sheet_text(harness: &Harness) -> String {
    harness.text_of(".ds-sheet").unwrap_or_default()
}

fn exported(requests: &Requests) -> Vec<ExportDraft> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| {
            if let HostRequest::Export(draft) = request {
                Some(*draft)
            } else {
                None
            }
        })
        .collect()
}

fn picture(dir: &std::path::Path) -> PathBuf {
    support::image_file(dir, "quadrants.png", "quadrants.png")
}

fn pdf(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("fixture.pdf");
    std::fs::write(&path, pdf_fixture::fixture_bytes()).unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// Whether each word is in the sheet's text, as `(word, wanted)` says.
fn shows(harness: &Harness, name: &str, words: &[(&str, bool)]) {
    let text = sheet_text(harness);
    for (word, wanted) in words {
        assert_eq!(
            text.contains(word),
            *wanted,
            "{name}: \"{word}\" should be {} in {text:?}",
            if *wanted { "shown" } else { "absent" }
        );
    }
}

#[test]
fn the_dialog_lists_the_formats_in_place_instead_of_a_drop_down_or_a_slide() {
    for scale in SCALES {
        the_dialog_lists_the_formats_in_place_instead_of_a_drop_down_or_a_slide_at(scale);
    }
}

fn the_dialog_lists_the_formats_in_place_instead_of_a_drop_down_or_a_slide_at(scale: u16) {
    let dir = tempfile::tempdir().unwrap();
    let (mut harness, _) = open(picture(dir.path()), scale);
    export(&mut harness);
    assert_eq!(harness.count(".ds-sheet"), 1);
    assert_eq!(
        harness.count(".ds-sheet[data-attach=\"centre\"]"),
        1,
        "it stands in the middle and fades in, it does not hang from the titlebar"
    );
    assert_eq!(
        harness.count(FORMATS),
        6,
        "PNG, JPEG, WebP, AVIF, TIFF and PDF are all listed"
    );
    assert_eq!(
        harness.count(".ds-sheet .ds-pop-up-button, .ds-menu"),
        0,
        "no drop-down anywhere in the dialog"
    );
    shows(
        &harness,
        "picture",
        &[
            ("PNG", true),
            ("JPEG", true),
            ("TIFF", true),
            ("All", true),
            ("No location", true),
            ("None", true),
            ("Keep metadata", false),
            // Every format says in a line what it is for, not only the chosen one.
            ("Sharp, and keeps transparency.", true),
            ("Smaller files, with no transparency.", true),
            ("Sharp and compact.", true),
            ("The smallest files, in a newer format.", true),
            ("Sharp, for editing and print.", true),
            ("One page holding the picture.", true),
            ("Saved beside the original as quadrants.png", true),
        ],
    );
}

#[test]
fn each_file_shows_the_controls_that_mean_something_for_it() {
    for scale in SCALES {
        each_file_shows_the_controls_that_mean_something_for_it_at(scale);
    }
}

fn each_file_shows_the_controls_that_mean_something_for_it_at(scale: u16) {
    let dir = tempfile::tempdir().unwrap();

    // A picture: size and metadata; quality only for a format that has it.
    let (mut harness, _) = open(picture(dir.path()), scale);
    export(&mut harness);
    shows(
        &harness,
        "png",
        &[("Size", true), ("Metadata", true), ("Quality", false)],
    );
    click(&mut harness, &format!("{FORMATS}:nth-child(2)"));
    shows(
        &harness,
        "jpeg",
        &[
            ("Quality", true),
            ("90%", true),
            ("Size", true),
            ("Saved beside the original as quadrants.jpg", true),
        ],
    );
    click(&mut harness, &format!("{FORMATS}:nth-child(6)"));
    shows(
        &harness,
        "picture as pdf",
        &[
            ("Quality", false),
            ("Size", false),
            ("One page holding the picture.", true),
        ],
    );

    // A PDF: pages first, and the picture options only once pictures are asked for.
    let (mut harness, _) = open(pdf(dir.path()), scale);
    export(&mut harness);
    shows(
        &harness,
        "pdf",
        &[
            ("Pages", true),
            ("Current page", true),
            ("Resolution", false),
        ],
    );
    assert_eq!(
        harness.count(".ds-sheet [aria-label=\"First page\"]"),
        0,
        "no page range until From – to is chosen"
    );
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Pages\"] .ds-segmented-segment:nth-child(3)",
    );
    assert!(
        harness.count(".ds-sheet [aria-label=\"First page\"]") > 0
            && harness.count(".ds-sheet [aria-label=\"Last page\"]") > 0,
        "the range appears"
    );
    click(&mut harness, &format!("{FORMATS}:nth-child(2)"));
    shows(
        &harness,
        "page images",
        &[
            ("Resolution", true),
            ("Format", true),
            ("Pages", true),
            ("Paper", false),
        ],
    );
    click(&mut harness, &format!("{FORMATS}:nth-child(3)"));
    shows(
        &harness,
        "plain text of a pdf",
        &[("Pages", false), ("Resolution", false)],
    );

    // Text: paper and orientation for the PDF, nothing for the text itself.
    let notes = text_file(dir.path(), "notes.txt", "word", 20);
    let (mut harness, _) = open(notes, scale);
    export(&mut harness);
    shows(
        &harness,
        "text as pdf",
        &[
            ("Paper", true),
            ("Orientation", true),
            ("Landscape", true),
            ("Size", false),
        ],
    );
    click(&mut harness, &format!("{FORMATS}:nth-child(2)"));
    shows(
        &harness,
        "plain text",
        &[
            ("Paper", false),
            ("Orientation", false),
            ("The text as it is.", true),
        ],
    );
}

#[test]
fn the_dialog_stands_the_same_size_and_centred_at_both_scales() {
    let dir = tempfile::tempdir().unwrap();
    let mut widths = Vec::new();
    for scale in [100, 200] {
        let (mut harness, _) = open(picture(dir.path()), scale);
        export(&mut harness);
        let rect = harness.rect(".ds-sheet").unwrap();
        let left = rect.origin.x.0;
        let right = 900.0 - (rect.origin.x.0 + rect.size.width.0);
        assert!(
            (left - right).abs() <= 1.0,
            "{scale}%: centred across the window: {rect:?}"
        );
        assert!(
            left >= 0.0 && rect.origin.y.0 >= 0.0,
            "{scale}%: on screen: {rect:?}"
        );
        assert!(
            rect.origin.y.0 + rect.size.height.0 <= 600.0,
            "{scale}%: within the window: {rect:?}"
        );
        // The formats and the options sit side by side in a dialog this wide.
        let formats = harness.rect(".viewer-export-formats").unwrap();
        let options = harness.rect(".viewer-export-options").unwrap();
        assert!(
            options.origin.x.0 >= formats.origin.x.0 + formats.size.width.0 - 1.0,
            "{scale}%: the options are beside the formats: {formats:?} {options:?}"
        );
        // Every button of the dialog is inside it.
        for selector in [
            ".viewer-sheet-buttons .ds-button:nth-child(1)",
            ".viewer-sheet-buttons .ds-button:nth-child(2)",
        ] {
            let button = harness.rect(selector).unwrap();
            assert!(
                button.origin.x.0 + button.size.width.0
                    <= rect.origin.x.0 + rect.size.width.0 + 1.0
                    && button.origin.y.0 + button.size.height.0
                        <= rect.origin.y.0 + rect.size.height.0 + 1.0,
                "{scale}%: {selector} is inside the dialog: {button:?} {rect:?}"
            );
        }
        widths.push(rect.size.width.0);
    }
    assert!(
        (widths[0] - widths[1]).abs() <= 1.0,
        "the same logical width at 100% and 200%: {widths:?}"
    );
}

#[test]
fn export_asks_the_host_for_the_options_chosen() {
    for scale in SCALES {
        export_asks_the_host_for_the_options_chosen_at(scale);
    }
}

fn export_asks_the_host_for_the_options_chosen_at(scale: u16) {
    let dir = tempfile::tempdir().unwrap();

    // A picture left as it opens: PNG, whole, and without where it was taken.
    let (mut harness, requests) = open(picture(dir.path()), scale);
    export(&mut harness);
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    assert_eq!(
        exported(&requests),
        [ExportDraft::Raster(RasterExport::Image(
            RasterTarget::Png,
            Resize::Original,
            MetadataCarry::StripLocation,
        ))],
        "the first thing a person exports does not carry the photo's location"
    );

    // A picture: JPEG, half size, no metadata.
    let (mut harness, requests) = open(picture(dir.path()), scale);
    export(&mut harness);
    click(&mut harness, &format!("{FORMATS}:nth-child(2)"));
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Size\"] .ds-segmented-segment:nth-child(2)",
    );
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Metadata\"] .ds-segmented-segment:nth-child(3)",
    );
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    assert_eq!(
        exported(&requests),
        [ExportDraft::Raster(RasterExport::Image(
            RasterTarget::Jpeg(Quality::clamped(Percent(90))),
            Resize::Scaled(Permille(500)),
            MetadataCarry::Drop,
        ))]
    );
    assert_eq!(harness.count(".ds-sheet"), 0, "and the dialog goes");

    // A PDF: the page images at 300 dpi.
    let (mut harness, requests) = open(pdf(dir.path()), scale);
    export(&mut harness);
    click(&mut harness, &format!("{FORMATS}:nth-child(2)"));
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Resolution\"] .ds-segmented-segment:nth-child(3)",
    );
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    assert_eq!(
        exported(&requests),
        [ExportDraft::Pdf(PdfExport::PageImages(
            PageSelection::All,
            RasterTarget::Png,
            Dpi::PRINT,
        ))]
    );

    // A PDF: just the page the reader is on.
    let (mut harness, requests) = open(pdf(dir.path()), scale);
    export(&mut harness);
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Pages\"] .ds-segmented-segment:nth-child(2)",
    );
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    assert_eq!(
        exported(&requests),
        [ExportDraft::Pdf(PdfExport::Pdf(PageSelection::Range(
            PageRange::single(PageIndex(0))
        )))]
    );

    // A text: landscape on Letter.
    let notes = text_file(dir.path(), "notes.txt", "word", 20);
    let (mut harness, requests) = open(notes, scale);
    export(&mut harness);
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Paper\"] .ds-segmented-segment:nth-child(3)",
    );
    click(
        &mut harness,
        ".ds-sheet [aria-label=\"Orientation\"] .ds-segmented-segment:nth-child(2)",
    );
    click(
        &mut harness,
        ".viewer-sheet-buttons .ds-button:nth-child(2)",
    );
    let sent = exported(&requests);
    assert!(
        matches!(
            sent[..],
            [ExportDraft::Text(TextExport::Pdf(layout))]
                if layout.paper == anyview_core::PaperSize::Letter
                    && layout.orientation == anyview_core::Orientation::Landscape
        ),
        "{sent:?}"
    );
}
