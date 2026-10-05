use super::css::tests::Files;
use super::*;
use std::collections::HashMap;

fn files() -> Files {
    Files(HashMap::from([
        ("OEBPS/img/a.png", "data:image/png;base64,AA"),
        (
            "OEBPS/style.css",
            "p { color: red } body { background: url(img/a.png) }",
        ),
    ]))
}

fn sealed(html: &str) -> Chapter {
    seal(html, "OEBPS/", &files())
}

#[test]
fn what_would_run_or_load_does_not_survive() {
    // name, html, text that must be absent
    const CASES: &[(&str, &str, &str)] = &[
        ("script", "<p>a</p><script>alert(1)</script>", "alert"),
        ("event handler", "<p onclick=\"x()\">a</p>", "onclick"),
        (
            "frame",
            "<iframe src=\"https://e.com\">fallback</iframe><p>a</p>",
            "iframe",
        ),
        (
            "frame contents",
            "<iframe src=\"https://e.com\">fallback</iframe><p>a</p>",
            "fallback",
        ),
        (
            "remote image",
            "<img src=\"https://e.com/a.png\" alt=\"x\">",
            "https://",
        ),
        (
            "form",
            "<form action=\"https://e.com\"><input name=a></form>",
            "<form",
        ),
        ("object", "<object data=\"x.swf\">fb</object>", "object"),
        (
            "javascript link",
            "<a href=\"javascript:x()\">l</a>",
            "javascript",
        ),
        (
            "remote css import",
            "<style>@import 'https://e.com/x.css'; p{}</style>",
            "e.com",
        ),
        (
            "remote css url",
            "<p style=\"background:url(https://e.com/a.png)\">a</p>",
            "e.com",
        ),
        (
            "remote stylesheet",
            "<link rel=\"stylesheet\" href=\"https://e.com/a.css\">",
            "e.com",
        ),
        (
            "svg script",
            "<svg><script>alert(1)</script></svg>",
            "alert",
        ),
        ("base", "<base href=\"https://e.com/\"><p>a</p>", "e.com"),
    ];
    for (name, html, absent) in CASES {
        let chapter = sealed(html);
        let all = format!("{}{}", chapter.styles, chapter.body);
        assert!(!all.contains(absent), "{name}: {all}");
    }
}

#[test]
fn a_chapter_keeps_its_content_its_images_and_its_styles() {
    let chapter = sealed(
        "<html><head><title>T</title><link rel=\"stylesheet\" href=\"style.css\"></head>\
         <body><h1 class=\"t\">Head &amp; tail</h1><p>Hi <em>there</em><img src=\"img/a.png\" alt=\"pic\"></p>\
         <a href=\"#n\">n</a></body></html>",
    );
    assert_eq!(
        chapter.body,
        "<h1 class=\"t\">Head &amp; tail</h1><p>Hi <em>there</em>\
         <img src=\"data:image/png;base64,AA\" alt=\"pic\"></p><a href=\"#n\">n</a>"
    );
    assert!(chapter.styles.contains("p { color: red }"));
    assert!(chapter.styles.contains("url(\"data:image/png;base64,AA\")"));
    assert!(!chapter.body.contains("<title"));
}

#[test]
fn an_svg_cover_becomes_a_picture_and_a_missing_image_its_alt() {
    let cover = sealed("<svg><image xlink:href=\"img/a.png\"/><text>no</text></svg>");
    assert_eq!(
        cover.body,
        "<img src=\"data:image/png;base64,AA\" style=\"max-width:100%\">"
    );
    let missing = sealed("<p><img src=\"gone.png\" alt=\"A map\"></p>");
    assert_eq!(missing.body, "<p>A map</p>");
}

#[test]
fn unbalanced_markup_is_closed_and_stray_closers_are_dropped() {
    let chapter = sealed("</div><div><p>one<p>two</span></div></div>");
    assert_eq!(chapter.body, "<div><p>one<p>two</p></p></div>");
}
