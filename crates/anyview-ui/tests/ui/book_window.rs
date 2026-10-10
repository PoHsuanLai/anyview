//! A book in the viewer window: an EPUB opens as the pages of a PDF, scrolls with the wheel like
//! one at the scales a desktop runs at, lists its chapters in Contents and goes to the one picked,
//! finds text in a chapter, shows thumbnails and opens where it was left; a comic is one page to a
//! picture. Both books are built here and opened as a person would.

use crate::support;

use anyview_core::{PageIndex, Permille, Resume, Zoom};
use anyview_ui::HostRequest;
use ds::prelude::{Appearance, Point, Px, ShortcutKey};
use ds_harness::{Driver, Harness, Input, Query, Viewport};
use std::io::Write;
use std::path::PathBuf;
use support::{Memory, Requests, Wiring, fixture, press, settle, shot, wired};

fn zip_at(dir: &std::path::Path, name: &str, entries: &[(&str, Vec<u8>)]) -> PathBuf {
    let path = dir.join(name);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (entry, bytes) in entries {
        writer
            .start_file(*entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    std::fs::canonicalize(path).unwrap()
}

fn picture() -> Vec<u8> {
    std::fs::read(fixture("anyview-image", "quadrants.png")).unwrap()
}

const CONTAINER: &str = r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
<rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#;

const PACKAGE: &str = r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Window Book</dc:title><dc:creator>Ann</dc:creator></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
<item id="b" href="b.xhtml" media-type="application/xhtml+xml"/>
<item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>
<item id="p" href="p.png" media-type="image/png"/></manifest>
<spine><itemref idref="a"/><itemref idref="b"/><itemref idref="c"/></spine></package>"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
<nav epub:type="toc"><ol><li><a href="a.xhtml">Alpha</a></li><li><a href="b.xhtml">Beta</a></li><li><a href="c.xhtml">Gamma</a></li></ol></nav></body></html>"#;

/// A chapter of `paragraphs` paragraphs, each a few lines, under a heading.
fn chapter(title: &str, paragraphs: usize, extra: &str) -> String {
    let paragraph = "The quick brown fox jumps over the lazy dog while the afternoon light \
        crosses the long room and settles on the open pages of an old book that nobody has \
        read aloud for many years, and the dust turns slowly in it.";
    let mut body = format!("<h1>{title}</h1>{extra}");
    for _ in 0..paragraphs {
        body.push_str(&format!("<p>{paragraph}</p>"));
    }
    format!("<html><body>{body}</body></html>")
}

fn epub(dir: &std::path::Path) -> PathBuf {
    zip_at(
        dir,
        "window.epub",
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("OEBPS/content.opf", PACKAGE.as_bytes().to_vec()),
            ("OEBPS/nav.xhtml", NAV.as_bytes().to_vec()),
            (
                "OEBPS/a.xhtml",
                chapter(
                    "Alpha",
                    40,
                    r#"<img src="p.png" alt="p"/><script>document.title='x'</script>"#,
                )
                .into_bytes(),
            ),
            (
                "OEBPS/b.xhtml",
                chapter("Beta", 30, "<p>The zeppelin arrives.</p>").into_bytes(),
            ),
            ("OEBPS/c.xhtml", chapter("Gamma", 12, "").into_bytes()),
            ("OEBPS/p.png", picture()),
        ],
    )
}

fn viewport(scale_percent: u16) -> Viewport {
    Viewport {
        width: 900,
        height: 600,
        scale_percent,
    }
}

fn open(path: &PathBuf, scale_percent: u16, wiring: Wiring) -> Harness {
    opened(path, scale_percent, wiring).0
}

fn opened(path: &PathBuf, scale_percent: u16, wiring: Wiring) -> (Harness, Requests) {
    let wiring = Wiring {
        viewport: Some(viewport(scale_percent)),
        ..wiring
    };
    let (mut harness, requests, _) =
        wired(std::slice::from_ref(path), 0, Appearance::default(), wiring);
    settle(&mut harness);
    (harness, requests)
}

fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

/// The page the capsule reads, and how many pages there are.
fn place(harness: &Harness) -> (u32, u32) {
    let text = harness.text_of(".ds-capsule-readout").unwrap_or_default();
    let (here, pages) = text.split_once(" of ").unwrap_or(("", ""));
    (here.parse().unwrap_or(0), pages.parse().unwrap_or(0))
}

fn middle(scale: u16) -> Point {
    let _ = scale;
    Point {
        x: Px(450.0),
        y: Px(300.0),
    }
}

/// Opens the panel's second tab (the first is Thumbnails).
fn open_tab(harness: &mut Harness, at: usize) {
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('i')));
    settle(harness);
    let tab = harness
        .centre(&format!(".ds-segmented-segment:nth-child({at})"))
        .unwrap();
    harness.send(Input::click(tab));
    settle(harness);
}

#[test]
fn an_epub_opens_as_the_pages_of_a_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = open(&epub(dir.path()), 100, Wiring::default());
    assert!(
        harness.count(".viewer-pdf-page") >= 1,
        "the first page is in the room"
    );
    assert!(
        harness.count(".viewer-pdf-tile") >= 1,
        "its tiles are drawn"
    );
    let (here, pages) = place(&harness);
    assert_eq!(here, 1, "{}", capsule(&harness));
    assert!(
        pages >= 6,
        "three chapters make several pages: {}",
        capsule(&harness)
    );
    assert_eq!(harness.count("iframe"), 0, "no frame swallows the wheel");
    if let Some(path) = shot("epub.png") {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn the_wheel_scrolls_an_epub_through_its_pages_at_each_scale() {
    for scale in [100u16, 200] {
        let dir = tempfile::tempdir().unwrap();
        let mut harness = open(&epub(dir.path()), scale, Wiring::default());
        let (_, pages) = place(&harness);
        let at = middle(scale);
        // Ten wheel clicks at a time: the page at the top only ever goes on, and the book is
        // read through to its last page.
        let mut seen = vec![place(&harness).0];
        for _ in 0..30 {
            harness.send(Input::detents(at, 0.0, -10.0));
            settle(&mut harness);
            seen.push(place(&harness).0);
        }
        assert!(
            seen.windows(2).all(|pair| pair[1] >= pair[0]),
            "{scale}%: the wheel only goes on: {seen:?}"
        );
        assert_eq!(
            seen.last().copied(),
            Some(pages),
            "{scale}%: to the last page: {seen:?}"
        );
        let mut back = vec![];
        for _ in 0..30 {
            harness.send(Input::detents(at, 0.0, 10.0));
            settle(&mut harness);
            back.push(place(&harness).0);
        }
        assert_eq!(
            back.last().copied(),
            Some(1),
            "{scale}%: and back to the first: {back:?}"
        );
    }
}

#[test]
fn the_panel_shows_a_thumbnail_for_each_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = open(&epub(dir.path()), 100, Wiring::default());
    let (_, pages) = place(&harness);
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('i')));
    settle(&mut harness);
    harness.send(Input::click(
        harness.centre(".ds-segmented-segment").unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-thumb"), pages as usize);
}

#[test]
fn contents_lists_the_chapters_and_each_leads_to_its_first_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = open(&epub(dir.path()), 100, Wiring::default());
    open_tab(&mut harness, 2);
    assert_eq!(
        harness.count(".viewer-outline-entry"),
        3,
        "Alpha, Beta and Gamma"
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-outline-entry:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    let beta = place(&harness).0;
    assert!(
        beta > 2,
        "Beta starts after Alpha's pages: {}",
        capsule(&harness)
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-outline-entry:nth-child(3)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert!(
        place(&harness).0 > beta,
        "Gamma follows Beta: {}",
        capsule(&harness)
    );
    harness.send(Input::click(
        harness
            .centre(".viewer-outline-entry:nth-child(1)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(place(&harness).0, 1, "Alpha is the first page");
}

#[test]
fn find_in_a_chapter_finds_its_text_and_goes_to_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = open(&epub(dir.path()), 100, Wiring::default());
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('f')));
    settle(&mut harness);
    for c in "zeppelin".chars() {
        harness.send(Input::key(ShortcutKey::Char(c)));
    }
    settle(&mut harness);
    let listed = harness.text_of(".ds-palette").unwrap_or_default();
    assert!(
        listed.contains("In This File") && listed.contains("Page"),
        "the one hit is listed with its page: {listed}"
    );
    assert!(
        place(&harness).0 > 2,
        "the hit is in Beta: {}",
        capsule(&harness)
    );
    assert!(
        harness.count(".viewer-pdf-hit") >= 1,
        "and it is marked on the page"
    );
}

#[test]
fn an_epub_opens_on_the_page_the_store_remembers_and_remembers_where_it_moves_to() {
    let dir = tempfile::tempdir().unwrap();
    let path = epub(dir.path());
    let memory = Memory::with(
        &path,
        Resume::Pdf {
            page: PageIndex(3),
            offset: Permille(0),
            zoom: Zoom::Fit,
        },
    );
    let wiring = Wiring {
        memory: Some(memory.clone()),
        ..Wiring::default()
    };
    let mut harness = open(&path, 100, wiring);
    assert_eq!(
        place(&harness).0,
        4,
        "it opened where it was left: {}",
        capsule(&harness)
    );
    press(&mut harness, ShortcutKey::Home);
    press(&mut harness, ShortcutKey::PageDown);
    let left = memory.left_at(&path);
    assert!(
        matches!(left, Some(Resume::Pdf { page, .. }) if page.0 <= 1),
        "{left:?}"
    );
}

fn comic(dir: &std::path::Path) -> PathBuf {
    zip_at(
        dir,
        "window.cbz",
        &[
            ("10.png", picture()),
            ("2.png", picture()),
            ("1.png", picture()),
        ],
    )
}

#[test]
fn a_comic_is_one_page_to_a_picture() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = open(&comic(dir.path()), 100, Wiring::default());
    assert_eq!(place(&harness), (1, 3), "{}", capsule(&harness));
    assert!(harness.count(".viewer-pdf-tile") >= 1);
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('i')));
    settle(&mut harness);
    harness.send(Input::click(
        harness.centre(".ds-segmented-segment").unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-thumb"), 3);
    press(&mut harness, ShortcutKey::End);
    assert_eq!(place(&harness).0, 3, "{}", capsule(&harness));
    if let Some(path) = shot("comic.png") {
        harness.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn a_book_offers_no_page_edits_for_its_pages_are_not_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut harness, requests) = opened(&epub(dir.path()), 100, Wiring::default());
    harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('k')));
    settle(&mut harness);
    let palette = harness
        .text_of(".ds-palette")
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        palette.contains("fit"),
        "the palette lists the page commands: {palette}"
    );
    for never in ["delete page", "move page", "export", "rotate"] {
        assert!(!palette.contains(never), "{never} in {palette}");
    }
    harness.send(Input::key(ShortcutKey::Escape));
    settle(&mut harness);
    for key in [ShortcutKey::Backspace, ShortcutKey::Down, ShortcutKey::Up] {
        harness.send(Input::chord(&[ShortcutKey::Super, ShortcutKey::Shift], key));
        settle(&mut harness);
    }
    let edits: Vec<HostRequest> = requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, HostRequest::Edit(_)))
        .cloned()
        .collect();
    assert!(edits.is_empty(), "{edits:?}");
}

#[test]
fn a_book_opens_at_reading_width_and_a_remembered_zoom_wins() {
    for scale in [100u16, 200] {
        let dir = tempfile::tempdir().unwrap();
        let harness = open(&epub(dir.path()), scale, Wiring::default());
        let page = harness.rect(".viewer-pdf-page").unwrap();
        assert!(
            page.size.width.0 >= 800.0 && page.size.width.0 <= 900.0,
            "{scale}%: the page fills the width of a 900 px window: {page:?}"
        );
        assert!(
            page.size.height.0 > 600.0,
            "{scale}%: so it is taller than the window: {page:?}"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = epub(dir.path());
    let memory = Memory::with(
        &path,
        Resume::Pdf {
            page: PageIndex(0),
            offset: Permille(0),
            zoom: Zoom::Fit,
        },
    );
    let harness = open(
        &path,
        100,
        Wiring {
            memory: Some(memory),
            ..Wiring::default()
        },
    );
    let page = harness.rect(".viewer-pdf-page").unwrap();
    assert!(
        page.size.height.0 <= 600.0,
        "a place left at fit-page stays: {page:?}"
    );
}

#[test]
fn the_export_dialog_is_never_offered_for_a_book_though_its_stage_is_the_pdf_stage() {
    let dir = tempfile::tempdir().unwrap();
    for scale in [100, 200] {
        let (mut harness, requests) = opened(&epub(dir.path()), scale, Wiring::default());
        harness.send(Input::chord(&[ShortcutKey::Super], ShortcutKey::Char('k')));
        settle(&mut harness);
        for letter in "export".chars() {
            harness.send(Input::key(ShortcutKey::Char(letter)));
        }
        settle(&mut harness);
        harness.send(Input::key(ShortcutKey::Enter));
        settle(&mut harness);
        assert_eq!(harness.count(".ds-sheet"), 0, "{scale}: no dialog opens");
        let exports = requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| matches!(request, HostRequest::Export(_)))
            .count();
        assert_eq!(exports, 0, "{scale}: nothing is exported");
    }
}
