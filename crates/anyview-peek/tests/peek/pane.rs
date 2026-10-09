//! The pane drawn: server-side renders of each body kept as goldens, the markup and stylesheet held
//! to quire's coherence rules, and behaviour through `ds_harness`'s real Blitz document.
//!
//! `DS_BLESS=1 cargo test -p anyview-peek --test peek pane::` writes the goldens; read the diff before
//! committing one.

#![cfg(feature = "pane")]

mod golden;
use crate::support;

use anyview_core::FormatKind;
use anyview_peek::{AnyPeeked, Body, PageLook, Pane, PdfPeeked, STYLE, peek};
use anyview_text::{TokenClass, TokenLine, TokenSpan};
use dioxus::core::VirtualDom;
use dioxus::prelude::*;
use ds::assembly::ds::Inject;
use ds::prelude::{Appearance, Ds, Material, Word};
use ds_harness::{Backdrop, Harness, HarnessConfig, Query, Viewport};
use ds_lint::{LintConfig, Profile, Rule, assert_clean, markup};
use std::sync::{Arc, OnceLock};
use support::{Home, fixture, pane_budget};

fn peeked(home: Home, name: &str) -> Arc<AnyPeeked> {
    let (src, sniffed) = fixture(home, name);
    Arc::new(peek(&src, &sniffed, &pane_budget()))
}

/// A PDF page with a made-up raster, so the golden does not hold pdfrum's pixels.
fn page() -> Arc<AnyPeeked> {
    let mut page = (*peeked(Home::Own, "hello.pdf")).clone();
    page.body = Body::Page(PdfPeeked {
        page: PageLook::Drawn {
            source: "data:image/png;base64,AAAA".to_owned(),
            width: 300,
            height: 200,
        },
    });
    Arc::new(page)
}

/// A code body with one span of every token class, so the stylesheet is held to cover them all.
fn every_token_class() -> Arc<AnyPeeked> {
    let mut code = (*peeked(Home::Text, "sample.rs")).clone();
    let spans = TokenClass::ALL
        .iter()
        .map(|class| TokenSpan {
            class: *class,
            text: class.slug().to_owned(),
        })
        .collect();
    let Body::Code(body) = &mut code.body else {
        panic!("a Rust file peeks to a Code body");
    };
    body.lines = vec![TokenLine {
        number: anyview_core::LineIndex(0),
        spans,
    }];
    Arc::new(code)
}

#[derive(Props, Clone, PartialEq)]
struct Setup {
    peeked: Arc<AnyPeeked>,
}

#[allow(non_snake_case)]
fn Root(setup: Setup) -> Element {
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            stylesheet: Inject::Host,
            Pane { peeked: setup.peeked }
        }
    }
}

/// The whole page's markup.
fn render(peeked: &Arc<AnyPeeked>) -> String {
    let mut dom = VirtualDom::new_with_props(
        Root,
        Setup {
            peeked: peeked.clone(),
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

/// The pane alone: from its opening tag to the close of its facts list, its last child.
fn pane(peeked: &Arc<AnyPeeked>) -> String {
    let html = render(peeked);
    let start = html.find("<div class=\"anyview-pane").unwrap();
    let end = html.rfind("</dl>").unwrap() + "</dl>".len();
    html[start..end].to_owned()
}

/// Every body the pane draws, peeked once for the whole binary: the goldens, the lint and the two
/// harness tests all look at the same ones.
fn body(name: &str) -> Arc<AnyPeeked> {
    static BODIES: OnceLock<Vec<(&str, Arc<AnyPeeked>)>> = OnceLock::new();
    let all = BODIES.get_or_init(|| {
        vec![
            ("picture", peeked(Home::Image, "quadrants.png")),
            ("page", page()),
            ("plain", peeked(Home::Text, "notes.txt")),
            ("code", peeked(Home::Text, "sample.rs")),
            ("every token class", every_token_class()),
            ("table", peeked(Home::Text, "people.csv")),
            ("tree", peeked(Home::Text, "config.json")),
            ("markdown", peeked(Home::Text, "readme.md")),
            ("archive", archive()),
            ("font", peeked(Home::Font, "blocks.ttf")),
            ("font-fallback", peeked(Home::Font, "circled.ttf")),
            ("unavailable", pdf_over_budget()),
            ("facts", facts_only()),
        ]
    });
    all.iter()
        .find(|(known, _)| *known == name)
        .unwrap_or_else(|| panic!("no body called {name}"))
        .1
        .clone()
}

#[test]
fn each_body_is_drawn_as_its_golden() {
    // name; the facts row is a kind without a back end, a plate and facts
    let names = [
        "picture",
        "page",
        "plain",
        "code",
        "table",
        "tree",
        "archive",
        "font",
        "font-fallback",
        "unavailable",
        "facts",
    ];
    assert_eq!(body("facts").kind, FormatKind::Other);
    let failures: Vec<String> = names
        .iter()
        .filter_map(|name| golden::check(&format!("pane/{name}.html"), &pane(&body(name))).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A zip of a folder and two files, peeked.
fn archive() -> Arc<AnyPeeked> {
    let dir = tempfile::tempdir().unwrap();
    let (src, sniffed) = support::zip_on_disk(
        dir.path(),
        &[
            ("docs/", ""),
            ("docs/readme.txt", "hello archive"),
            ("data.bin", "0123456789"),
        ],
    );
    Arc::new(peek(&src, &sniffed, &pane_budget()))
}

/// A file the peek refuses, so the pane shows the reason.
fn pdf_over_budget() -> Arc<AnyPeeked> {
    let (src, sniffed) = fixture(Home::Own, "hello.pdf");
    Arc::new(peek(&src, &sniffed, &support::budget(100, 1_000_000)))
}

/// The kinds with no body of their own: facts under a plate.
fn facts_only() -> Arc<AnyPeeked> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blob.bin");
    std::fs::write(&path, b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0").unwrap();
    let (src, sniffed) = support::on_disk(&path, 0);
    Arc::new(peek(&src, &sniffed, &pane_budget()))
}

#[test]
fn the_stylesheet_uses_design_system_tokens_only() {
    assert_clean(
        STYLE,
        &LintConfig {
            profile: Profile::Strict,
            ..LintConfig::new(&ds::kits())
        },
    );
}

/// quire's `TextureLayer` carries `ds-texture-layer` and styles itself inline, so its own class has
/// no rule: quire's markup lint flags quire's own component (FINDINGS: open item for quire).
fn is_texture_layer(offence: &ds_lint::Offence) -> bool {
    offence.selector == "object.ds-texture-layer"
}

#[test]
fn every_class_the_pane_draws_is_styled_and_nothing_is_raw_markup() {
    let css = format!("{}\n{STYLE}", ds::stylesheet());
    let all = [
        "picture",
        "page",
        "plain",
        "every token class",
        "table",
        "tree",
        "markdown",
        "archive",
        "font",
        "facts",
        "unavailable",
    ]
    .map(body);
    for peeked in &all {
        let html = render(peeked);
        let offences = markup(&html, &css, &LintConfig::new(&ds::kits()));
        let wrong: Vec<_> = offences
            .iter()
            .filter(|offence| {
                matches!(
                    offence.rule,
                    Rule::UnstyledClass | Rule::RawMarkup | Rule::HexColour | Rule::RawDuration
                ) && !is_texture_layer(offence)
            })
            .collect();
        assert!(wrong.is_empty(), "{}: {wrong:#?}", peeked.body.slug());
    }
}

const VIEW: Viewport = Viewport {
    width: 360,
    height: 900,
    scale_percent: 100,
};

fn app() -> Element {
    let peeked = use_context::<Arc<AnyPeeked>>();
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            Pane { peeked }
        }
    }
}

fn harness(peeked: Arc<AnyPeeked>) -> Harness {
    Harness::new(app, HarnessConfig::new(VIEW).with_context(peeked))
}

#[test]
fn the_pane_in_a_real_document_lists_the_file_and_its_facts() {
    // name, what was peeked, lines or rows drawn, facts listed
    let cases: Vec<(&str, Arc<AnyPeeked>, &str, usize, usize)> = vec![
        ("code", body("code"), ".anyview-line", 16, 5),
        ("plain", body("plain"), ".anyview-line", 40, 5),
        ("table", body("table"), ".ds-table-cell", 40 * 3, 6),
        ("archive", body("archive"), ".ds-table-cell", 3 * 2, 4),
        ("font", body("font"), ".anyview-specimen-line", 3, 6),
    ];
    for (name, peeked, selector, drawn, facts) in cases {
        let file = peeked.name.clone();
        let harness = harness(peeked);
        assert_eq!(
            harness.text_of(".anyview-pane-title").as_deref(),
            Some(file.as_str()),
            "{name}"
        );
        assert_eq!(harness.count(selector), drawn, "{name}");
        assert_eq!(harness.count(".ds-fact-list-item"), facts, "{name}");
    }
}

#[test]
fn the_specimen_is_inked_in_a_real_document_with_the_panes_ink() {
    let mut harness = harness(body("font"));
    let shot = harness.render_over(Backdrop::Clear).unwrap();
    let rect = harness.rect(".anyview-specimen-line").unwrap();
    assert!(
        rect.size.width.0 > 100.0 && rect.size.height.0 > 20.0,
        "{rect:?}"
    );
    let (left, top) = (rect.origin.x.0 as u32, rect.origin.y.0 as u32);
    let (width, height) = (rect.size.width.0 as u32, rect.size.height.0 as u32);
    let ground = shot.get_pixel(left, top).0;
    let inked = (top..top + height)
        .flat_map(|y| (left..left + width).map(move |x| (x, y)))
        .filter(|(x, y)| shot.get_pixel(*x, *y).0 != ground)
        .count();
    // Capital letters cover a good part of their line's box.
    assert!(
        inked > (width * height) as usize / 20,
        "{inked} inked of {}",
        width * height
    );
}

/// The launcher's box for the pane: its column inside quire's padding.
const HOST: Viewport = Viewport {
    width: 328,
    height: 318,
    scale_percent: 100,
};

fn hosted() -> Element {
    let peeked = use_context::<Arc<AnyPeeked>>();
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            div {
                id: "host",
                style: "position:relative;display:flex;flex-direction:column;width:328px;height:318px;overflow:hidden",
                Pane { peeked }
            }
        }
    }
}

#[test]
fn the_pane_fits_the_box_it_is_given_and_shows_its_name_and_every_fact() {
    let names = [
        "picture", "page", "plain", "code", "table", "tree", "markdown", "archive", "font", "facts",
    ];
    for name in names {
        let peeked = body(name);
        let config = HarnessConfig::new(Viewport {
            width: 360,
            height: 700,
            scale_percent: 100,
        })
        .with_context(peeked);
        let harness = Harness::new(hosted, config);
        let host = harness.rect("#host").unwrap();
        let inside = |selector: &str| {
            let rect = harness
                .rect(selector)
                .unwrap_or_else(|| panic!("{name}: no {selector}"));
            let bottom = rect.origin.y.0 + rect.size.height.0;
            let right = rect.origin.x.0 + rect.size.width.0;
            assert!(
                bottom <= host.origin.y.0 + host.size.height.0 + 0.5,
                "{name}: {selector} ends at {bottom}, the host at {}",
                host.origin.y.0 + host.size.height.0
            );
            assert!(
                right <= host.origin.x.0 + host.size.width.0 + 0.5,
                "{name}: {selector} reaches {right}"
            );
            rect
        };
        let pane = inside(".anyview-pane");
        assert!(
            (pane.size.height.0 - f32::from(HOST.height as u16)).abs() < 1.0,
            "{name}: the pane is {} tall in a {} tall box",
            pane.size.height.0,
            HOST.height
        );
        let media = inside(".anyview-pane-media");
        assert!(
            media.size.height.0 > 40.0,
            "{name}: the media box is {}",
            media.size.height.0
        );
        inside(".anyview-pane-title");
        // Every fact, not only the first, lies inside the host: the media box gave way to them.
        inside(".ds-fact-list");
    }
}
