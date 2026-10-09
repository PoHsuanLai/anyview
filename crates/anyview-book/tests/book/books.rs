//! A tiny EPUB and a tiny comic, built in the test, read through the public API.

use crate::support;

use anyview_book::{Comic, Epub, comic_cover, epub_cover, natural_order};
use anyview_core::SectionIndex;

#[test]
fn an_epub_reads_its_metadata_spine_and_cover() {
    let dir = tempfile::tempdir().unwrap();
    let epub = Epub::open(&support::epub(dir.path())).unwrap();
    let meta = epub.meta();
    assert_eq!(meta.title.as_deref(), Some("The Test Book"));
    assert_eq!(meta.author.as_deref(), Some("Ann Author, Bo Writer"));
    assert_eq!(meta.publisher.as_deref(), Some("Small Press"));
    assert_eq!(meta.language.as_deref(), Some("en"));
    // The spine says one then two though the manifest lists two first.
    assert_eq!(epub.sections().get(), 2);
    let first = epub.chapter(SectionIndex(0)).unwrap();
    assert!(first.body.contains("Chapter One"));
    let second = epub.chapter(SectionIndex(1)).unwrap();
    assert!(
        second.body.contains("Second\u{a0}chapter") || second.body.contains("Second&nbsp;chapter")
    );
    let cover = epub_cover(&epub).unwrap();
    assert_eq!(
        (cover.name.as_str(), cover.bytes.as_slice()),
        ("cover.png", support::PNG)
    );
}

#[test]
fn the_contents_nest_and_lead_to_chapters() {
    let dir = tempfile::tempdir().unwrap();
    let epub = Epub::open(&support::epub(dir.path())).unwrap();
    let got: Vec<(String, u8, Option<u32>)> = epub
        .contents()
        .into_iter()
        .map(|entry| (entry.title, entry.depth, entry.section.map(|s| s.0)))
        .collect();
    let want = vec![
        ("One".to_owned(), 0, Some(0)),
        ("One, part".to_owned(), 1, Some(0)),
        ("Two".to_owned(), 0, Some(1)),
    ];
    assert_eq!(got, want);
}

#[test]
fn a_sealed_chapter_inlines_what_the_package_holds_and_nothing_remote() {
    let dir = tempfile::tempdir().unwrap();
    let epub = Epub::open(&support::epub(dir.path())).unwrap();
    let chapter = epub.chapter(SectionIndex(0)).unwrap();
    let data = format!(
        "data:image/png;base64,{}",
        ds_core::base64::encode(support::PNG)
    );
    assert!(
        chapter
            .body
            .contains(&format!("<img src=\"{data}\" alt=\"pic\">")),
        "{}",
        chapter.body
    );
    assert!(chapter.styles.contains("h1 { color: blue }"));
    assert!(chapter.styles.contains(&format!("url(\"{data}\")")));
    let everything = format!("{}{}", chapter.styles, chapter.body);
    for forbidden in ["example.com", "<script", "alert", "<link", "http"] {
        assert!(!everything.contains(forbidden), "{forbidden} survived");
    }
    assert!(
        chapter.body.contains("remote"),
        "the remote image keeps its alt text"
    );
    assert!(epub.chapter(SectionIndex(2)).is_err());
}

#[test]
fn a_comic_lists_its_images_in_natural_order_and_skips_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = support::comic(dir.path());
    let comic = Comic::open(&path).unwrap();
    let entries: Vec<&str> = comic
        .pages()
        .iter()
        .map(|page| page.entry.as_str())
        .collect();
    assert_eq!(entries, ["pages/1.png", "pages/2.png", "pages/10.png"]);
    assert_eq!(comic.sections().get(), 3);
    let (page, bytes) = comic.page(SectionIndex(2)).unwrap();
    assert_eq!((page.mime, bytes.as_slice()), ("image/png", support::PNG));
    assert!(comic.page(SectionIndex(3)).is_err());
    assert_eq!(comic_cover(&path).unwrap().name, "1.png");
    assert_eq!(
        natural_order("pages/2.png", "pages/10.png"),
        std::cmp::Ordering::Less
    );
}

#[test]
fn a_zip_without_chapters_or_pages_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = support::write_zip(dir.path(), "none.cbz", &[("a.txt", b"x")]);
    assert!(Comic::open(&path).is_err());
}
