use super::*;
use crate::code::Highlighter;
use anyview_core::FilePath;
use std::collections::HashMap;

struct Files(HashMap<String, Vec<u8>>);

impl LocalFiles for Files {
    fn read(&self, path: &FilePath, most: usize) -> Option<Vec<u8>> {
        let bytes = self.0.get(path.as_path().to_str()?)?;
        (bytes.len() <= most).then(|| bytes.clone())
    }
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn plain(source: &str) -> Rendered {
    render(
        source,
        &RenderEnv {
            base: None,
            files: &NoFiles,
            highlighter: None,
        },
    )
}

fn with_files(source: &str) -> Rendered {
    let files = Files(HashMap::from([("/docs/pic.png".to_owned(), PNG.to_vec())]));
    let base = FilePath::new("/docs").unwrap();
    render(
        source,
        &RenderEnv {
            base: Some(&base),
            files: &files,
            highlighter: None,
        },
    )
}

fn outline(rendered: &Rendered) -> Vec<(u8, &str, &str)> {
    rendered
        .outline
        .iter()
        .map(|h| (h.level.get(), h.text.as_str(), h.anchor.as_str()))
        .collect()
}

#[test]
fn a_heading_and_a_paragraph_render_with_an_anchor_and_an_outline_entry() {
    let rendered = plain("# Title\n\nHello *world*.\n");
    assert_eq!(
        rendered.html,
        "<h1 id=\"title\">Title</h1>\n<p>Hello <em>world</em>.</p>\n"
    );
    assert_eq!(outline(&rendered), [(1, "Title", "title")]);
}

#[test]
fn github_extensions_render_tables_task_lists_and_strikethrough() {
    // name, source, fragment the html must contain
    const CASES: &[(&str, &str, &str)] = &[
        (
            "table head",
            "| a | b |\n|---|---|\n| 1 | 2 |\n",
            "<th>a</th>",
        ),
        (
            "table cell",
            "| a | b |\n|---|---|\n| 1 | 2 |\n",
            "<td>2</td>",
        ),
        (
            "done task",
            "- [x] done\n",
            "<input disabled=\"\" type=\"checkbox\" checked=\"\"/>",
        ),
        (
            "open task",
            "- [ ] todo\n",
            "<input disabled=\"\" type=\"checkbox\"/>",
        ),
        ("strikethrough", "~~gone~~\n", "<del>gone</del>"),
    ];
    for (name, source, fragment) in CASES {
        let html = plain(source).html;
        assert!(html.contains(fragment), "{name}: {html}");
    }
}

#[test]
fn raw_html_is_shown_as_text_and_never_passed_through() {
    // name, source, fragment present, fragment absent
    const CASES: &[(&str, &str, &str, &str)] = &[
        (
            "script block",
            "<script>alert(1)</script>\n",
            "&lt;script&gt;alert(1)&lt;/script&gt;",
            "<script",
        ),
        (
            "inline tag",
            "text <b>x</b> more\n",
            "text &lt;b&gt;x&lt;/b&gt; more",
            "<b>",
        ),
        (
            "div block",
            "<div>hi</div>\n",
            "<pre class=\"raw-html\"><code>&lt;div&gt;hi&lt;/div&gt;\n</code></pre>",
            "<div>",
        ),
        (
            "comment",
            "a <!-- note --> b\n",
            "&lt;!-- note --&gt;",
            "<!--",
        ),
        (
            "onerror image",
            "<img src=x onerror=alert(1)>\n",
            "&lt;img src=x onerror=alert(1)&gt;",
            "<img",
        ),
    ];
    for (name, source, present, absent) in CASES {
        let html = plain(source).html;
        assert!(
            html.contains(present),
            "{name} should contain {present:?}: {html}"
        );
        assert!(
            !html.contains(absent),
            "{name} should not contain {absent:?}: {html}"
        );
    }
}

#[test]
fn link_targets_are_vetted() {
    // name, source, href
    const CASES: &[(&str, &str, &str)] = &[
        (
            "https",
            "[x](https://example.com/a)\n",
            "href=\"https://example.com/a\"",
        ),
        ("relative", "[x](docs/guide.md)\n", "href=\"docs/guide.md\""),
        ("anchor", "[x](#install)\n", "href=\"#install\""),
        ("javascript", "[x](javascript:alert(1))\n", "href=\"#\""),
        ("data", "[x](data:text/html,hi)\n", "href=\"#\""),
    ];
    for (name, source, href) in CASES {
        let html = plain(source).html;
        assert!(html.contains(href), "{name}: {html}");
    }
}

#[test]
fn local_images_are_inlined_and_the_rest_fall_back_to_their_alt_text() {
    let inlined = with_files("![a pic](pic.png)\n").html;
    assert!(
        inlined.contains("<img src=\"data:image/png;base64,iVBORw0KGgo"),
        "{inlined}"
    );
    assert!(inlined.contains("alt=\"a pic\""), "{inlined}");

    let missing = with_files("![gone](nothing.png)\n").html;
    assert_eq!(
        missing,
        "<p><span class=\"missing-image\">gone</span></p>\n"
    );

    let remote = with_files("![far](https://example.com/y.png)\n").html;
    assert!(
        remote.contains("<span class=\"missing-image\">far</span>"),
        "{remote}"
    );
    assert!(!remote.contains("example.com"), "{remote}");

    let without_reader = plain("![a pic](pic.png)\n").html;
    assert!(without_reader.contains("missing-image"), "{without_reader}");
}

#[test]
fn headings_are_listed_in_order_with_unique_anchors_and_plain_text() {
    let source = "# Intro\n\n## Setup\n\ntext\n\n## Setup\n\n### Use `foo` *now*\n";
    let rendered = plain(source);
    assert_eq!(
        outline(&rendered),
        [
            (1, "Intro", "intro"),
            (2, "Setup", "setup"),
            (2, "Setup", "setup-1"),
            (3, "Use foo now", "use-foo-now"),
        ]
    );
    for heading in &rendered.outline {
        let id = format!("id=\"{}\"", heading.anchor.as_str());
        assert!(rendered.html.contains(&id), "{id} in {}", rendered.html);
    }
}

#[test]
fn fenced_code_is_highlighted_when_a_highlighter_is_given_and_plain_otherwise() {
    let highlighter = Highlighter::new();
    let source = "```rust\nlet x = 1;\n```\n";
    let env = RenderEnv {
        base: None,
        files: &NoFiles,
        highlighter: Some(&highlighter),
    };
    let html = render(source, &env).html;
    assert!(
        html.starts_with("<pre><code class=\"language-rust\">"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"tok-keyword\">let</span>"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"tok-number\">1</span>"),
        "{html}"
    );

    let unhighlighted = plain(source).html;
    assert_eq!(
        unhighlighted,
        "<pre><code class=\"language-rust\">let x = 1;\n</code></pre>\n"
    );

    let unknown = render("```klingon\nqapla'\n```\n", &env).html;
    assert_eq!(
        unknown,
        "<pre><code class=\"language-klingon\">qapla'\n</code></pre>\n"
    );
}

#[test]
fn a_fenced_label_may_be_a_name_an_extension_or_have_trailing_words() {
    let highlighter = Highlighter::new();
    let env = RenderEnv {
        base: None,
        files: &NoFiles,
        highlighter: Some(&highlighter),
    };
    for label in ["rust", "Rust", "rs", "rust,no_run", "rs title=\"x\""] {
        let html = render(&format!("```{label}\nlet x = 1;\n```\n"), &env).html;
        assert!(html.contains("tok-keyword"), "{label}: {html}");
    }
}

#[test]
fn an_empty_document_renders_nothing() {
    let rendered = plain("");
    assert_eq!(rendered.html, "");
    assert!(rendered.outline.is_empty());
}

#[test]
fn an_image_that_names_a_device_is_shown_as_its_alt_text_and_never_read_to_the_end() {
    let rendered = render(
        "![the void](/dev/zero) and ![fifty](/dev/urandom)\n",
        &RenderEnv {
            base: None,
            files: &DiskFiles,
            highlighter: None,
        },
    );
    assert_eq!(
        rendered
            .html
            .matches("<span class=\"missing-image\">")
            .count(),
        2
    );
    assert!(!rendered.html.contains("data:"));
}

#[test]
fn a_document_that_names_one_large_image_again_and_again_stops_inlining_it() {
    let dir = tempfile::tempdir().unwrap();
    let big = [PNG, &vec![0; 6 * 1024 * 1024]].concat();
    std::fs::write(dir.path().join("big.png"), big).unwrap();
    let base = FilePath::new(dir.path()).unwrap();
    let source = "![a](big.png)\n\n".repeat(1000);
    let rendered = render(
        &source,
        &RenderEnv {
            base: Some(&base),
            files: &DiskFiles,
            highlighter: None,
        },
    );
    let inlined = rendered.html.matches("<img").count();
    assert!((1..10).contains(&inlined), "{inlined} copies");
    assert!(rendered.html.len() < 48 * 1024 * 1024);
}
