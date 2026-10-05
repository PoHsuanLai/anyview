//! A book in the window: an EPUB chapter is a sealed frame with its text and its inlined picture,
//! the page keys move between chapters, the contents panel leads to one, and a comic shows its pages
//! in natural order. Both books are built here and opened as a person would.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{Resume, SectionIndex};
use ds::prelude::{Appearance, ShortcutKey};
use ds_harness::Input;
use ds_harness::{Driver, Harness, Query};
use std::io::Write;
use std::path::PathBuf;
use support::{Memory, Wiring, fixture, press, settle, shot, wired};

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
    path
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
<item id="p" href="p.png" media-type="image/png"/></manifest>
<spine><itemref idref="a"/><itemref idref="b"/></spine></package>"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
<nav epub:type="toc"><ol><li><a href="a.xhtml">Alpha</a></li><li><a href="b.xhtml">Beta</a></li></ol></nav></body></html>"#;

const CHAPTER_A: &str = r#"<html><body><h1>Alpha chapter</h1><p>First text.</p><img src="p.png" alt="p"/>
<script>document.title='x'</script><img src="https://example.com/r.png"/></body></html>"#;

const CHAPTER_B: &str = "<html><body><h1>Beta chapter</h1><p>Second text.</p></body></html>";

fn epub(dir: &std::path::Path) -> PathBuf {
    zip_at(
        dir,
        "window.epub",
        &[
            ("mimetype", b"application/epub+zip".to_vec()),
            ("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
            ("OEBPS/content.opf", PACKAGE.as_bytes().to_vec()),
            ("OEBPS/nav.xhtml", NAV.as_bytes().to_vec()),
            ("OEBPS/a.xhtml", CHAPTER_A.as_bytes().to_vec()),
            ("OEBPS/b.xhtml", CHAPTER_B.as_bytes().to_vec()),
            ("OEBPS/p.png", picture()),
        ],
    )
}

fn frame_text(harness: &Harness, selector: &str) -> Option<String> {
    harness.frame("iframe.viewer-frame")?.text_of(selector)
}

fn capsule(harness: &Harness) -> String {
    harness.text_of(".ds-capsule").unwrap_or_default()
}

#[test]
fn an_epub_chapter_is_a_sealed_frame_and_the_page_keys_move_between_chapters() {
    let dir = tempfile::tempdir().unwrap();
    let paths = vec![epub(dir.path())];
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    assert_eq!(harness.count("iframe.viewer-frame"), 1);
    assert_eq!(frame_text(&harness, "h1").as_deref(), Some("Alpha chapter"));
    let page = harness.frame("iframe.viewer-frame").unwrap();
    assert_eq!(
        page.count("img"),
        1,
        "the held picture shows, the remote one does not"
    );
    assert_eq!(page.count("script"), 0);
    assert!(capsule(&harness).contains("1 / 2"), "{}", capsule(&harness));
    settle(&mut harness);
    if let Some(path) = shot("epub.png") {
        harness.render().unwrap().save(path).unwrap();
    }
    press(&mut harness, ShortcutKey::PageDown);
    assert_eq!(frame_text(&harness, "h1").as_deref(), Some("Beta chapter"));
    assert!(capsule(&harness).contains("2 / 2"), "{}", capsule(&harness));
    press(&mut harness, ShortcutKey::PageDown);
    assert_eq!(
        frame_text(&harness, "h1").as_deref(),
        Some("Beta chapter"),
        "the last chapter stays"
    );
    press(&mut harness, ShortcutKey::PageUp);
    assert_eq!(frame_text(&harness, "h1").as_deref(), Some("Alpha chapter"));
}

#[test]
fn an_epub_opens_on_the_chapter_the_store_remembers_and_remembers_the_one_it_moves_to() {
    let dir = tempfile::tempdir().unwrap();
    let paths = vec![epub(dir.path())];
    let memory = Memory::with(
        &paths[0],
        Resume::Book {
            section: SectionIndex(1),
        },
    );
    let wiring = Wiring {
        memory: Some(memory.clone()),
        ..Wiring::default()
    };
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), wiring);
    settle(&mut harness);
    assert_eq!(frame_text(&harness, "h1").as_deref(), Some("Beta chapter"));
    press(&mut harness, ShortcutKey::PageUp);
    assert_eq!(
        memory.left_at(&paths[0]),
        Some(Resume::Book {
            section: SectionIndex(0)
        })
    );
}

#[test]
fn the_contents_panel_lists_the_chapters_and_each_leads_to_its_chapter() {
    let dir = tempfile::tempdir().unwrap();
    let paths = vec![epub(dir.path())];
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    harness.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('i')));
    settle(&mut harness);
    // ⌘I shows the Info tab; the contents are the first of the two.
    harness.send(Input::click(
        harness.centre(".ds-segmented-segment").unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(harness.count(".viewer-outline-entry"), 2, "Alpha and Beta");
    harness.send(Input::click(
        harness
            .centre(".viewer-outline-entry:nth-child(2)")
            .unwrap(),
    ));
    settle(&mut harness);
    assert_eq!(frame_text(&harness, "h1").as_deref(), Some("Beta chapter"));
    assert!(capsule(&harness).contains("2 / 2"), "{}", capsule(&harness));
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
fn a_comic_shows_one_page_at_a_time_in_natural_order() {
    let dir = tempfile::tempdir().unwrap();
    let paths = vec![comic(dir.path())];
    let (mut harness, _, _) = wired(&paths, 0, Appearance::default(), Wiring::default());
    settle(&mut harness);
    let page = harness.frame("iframe.viewer-frame").unwrap();
    assert_eq!(page.count("img.viewer-comic-page"), 1);
    assert!(capsule(&harness).contains("1 / 3"), "{}", capsule(&harness));
    settle(&mut harness);
    if let Some(path) = shot("comic.png") {
        harness.render().unwrap().save(path).unwrap();
    }
    press(&mut harness, ShortcutKey::End);
    assert!(capsule(&harness).contains("3 / 3"), "{}", capsule(&harness));
    press(&mut harness, ShortcutKey::Home);
    assert!(capsule(&harness).contains("1 / 3"), "{}", capsule(&harness));
}
