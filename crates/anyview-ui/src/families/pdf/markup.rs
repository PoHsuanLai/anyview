//! The PDF stage's own markup, rendered server-side inside a `Ds` root and compared with a golden
//! under `tests/snapshots/pdf/` (`DS_BLESS=1` rewrites them; read the diff), and linted against
//! quire's stylesheet and the viewer's. The tiles are `TextureLayer`s, which need a GPU, so the
//! window test (`tests/pdf_window.rs`) is what shows them; here the page boxes, the find bar and
//! the panel's lists are drawn without textures.

use super::doc::PdfDoc;
use super::find::PdfFinding;
use super::page::{Emphasis, LinkView, Mark, PageBox};
use super::panel::{Outline, Thumbnails};
use super::shelf::use_pdf_shelf;
use crate::families::view::{FrameLook, Held, StageCx};
use crate::testing::golden;
use crate::{FindHits, HitCount, HitIndex, PageView, PdfStage, Stage, StageIn, Ticket, TypedText};
use anyview_core::{PageIndex, Permille, Zoom};
use anyview_pdf::{LinkTarget, PdfDocument};
use dioxus::prelude::*;
use ds::assembly::ds::Inject;
use ds::prelude::*;
use ds_lint::{LintConfig, markup};
use std::sync::Arc;

#[path = "../../../../anyview-pdf/tests/pdf/support/mod.rs"]
mod fixture;

fn doc() -> Arc<PdfDoc> {
    let document = PdfDocument::from_bytes(fixture::fixture_bytes()).unwrap();
    Arc::new(PdfDoc::of(document))
}

fn view() -> PageView {
    PageView {
        page: PageIndex(0),
        offset: Permille(0),
        zoom: Zoom::Fit,
    }
}

fn finding() -> Stage {
    Stage::Pdf(PdfStage::Finding {
        query: TypedText::new("fox"),
        hits: FindHits::answered(HitCount(3), HitIndex(1)),
        view: view(),
    })
}

fn cx(stage: Stage) -> StageCx {
    StageCx {
        stage,
        hand: crate::Hand::default(),
        ticket: Ticket::default(),
        area: None,
        send: EventHandler::new(|_: StageIn| {}),
        run: EventHandler::new(|_| {}),
        lines: None,
        ask_lines: EventHandler::new(|_| {}),
        typing: EventHandler::new(|_| {}),
        hits: None,
        work: EventHandler::new(|_| {}),
        request: EventHandler::new(|_| {}),
        pdf: use_pdf_shelf(),
        media: crate::families::use_media_shelf(),
        frame: FrameLook::default(),
        platform: crate::PlatformAbilities::ALL,
    }
}

fn root(children: Element) -> Element {
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            stylesheet: Inject::Host,
            {children}
        }
    }
}

/// One component in one state.
struct Case {
    name: &'static str,
    make: fn() -> Element,
}

const CASES: &[Case] = &[
    Case {
        name: "page-marked",
        make: || {
            let marks = vec![
                Mark {
                    style: "left:11.8%;top:18.0%;width:37.9%;height:2.0%".to_owned(),
                    emphasis: Emphasis::Current,
                },
                Mark {
                    style: "left:52.0%;top:18.0%;width:8.0%;height:2.0%".to_owned(),
                    emphasis: Emphasis::Other,
                },
            ];
            let links = vec![
                LinkView {
                    style: "left:11.8%;top:15.0%;width:37.0%;height:2.5%".to_owned(),
                    target: LinkTarget::Page(PageIndex(2)),
                },
                LinkView {
                    style: "left:11.8%;top:24.0%;width:20.0%;height:2.5%".to_owned(),
                    target: LinkTarget::Uri("https://example.com/".to_owned()),
                },
            ];
            root(rsx! {
                div { style: "position:relative; width:300px; height:400px",
                    PageBox {
                        number: 1,
                        style: "left:10px;top:10px;width:280px;height:380px".to_owned(),
                        tiles: Vec::new(),
                        marks,
                        links,
                        onlink: |_| {},
                    }
                }
            })
        },
    },
    Case {
        name: "find-bar-on-the-second-of-three",
        make: || {
            let cx = cx(finding());
            root(
                rsx! { div { style: "position:relative; width:600px; height:120px", PdfFinding { cx } } },
            )
        },
    },
    Case {
        name: "find-bar-with-no-match",
        make: || {
            let stage = Stage::Pdf(PdfStage::Finding {
                query: TypedText::new("zebra"),
                hits: FindHits::answered(HitCount(0), HitIndex(0)),
                view: view(),
            });
            let cx = cx(stage);
            root(
                rsx! { div { style: "position:relative; width:600px; height:120px", PdfFinding { cx } } },
            )
        },
    },
    Case {
        name: "thumbnails",
        make: || {
            let cx = cx(Stage::Pdf(PdfStage::Reading { view: view() }));
            root(rsx! { Thumbnails { doc: Held(doc()), cx } })
        },
    },
    Case {
        name: "outline",
        make: || {
            let cx = cx(Stage::Pdf(PdfStage::Reading { view: view() }));
            root(rsx! { Outline { doc: Held(doc()), cx } })
        },
    },
];

#[derive(Props, Clone)]
struct HostProps {
    make: fn() -> Element,
}

impl PartialEq for HostProps {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

fn host(props: HostProps) -> Element {
    (props.make)()
}

fn render(case: &Case) -> String {
    let mut dom = VirtualDom::new_with_props(host, HostProps { make: case.make });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn every_pdf_view_matches_its_golden() {
    let failures: Vec<String> = CASES
        .iter()
        .filter_map(|case| golden::check(&format!("pdf/{}.html", case.name), &render(case)).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_pdf_view_lints_clean_against_both_stylesheets() {
    let css = format!("{}\n{}", ds::stylesheet(), crate::views::stylesheet());
    for case in CASES {
        // quire's `TextureLayer` styles itself inline and its class has no rule (FINDINGS, open
        // item for quire): its own component is let through and nothing else.
        let offences: Vec<_> = markup(&render(case), &css, &LintConfig::new(&ds::kits()))
            .into_iter()
            .filter(|offence| offence.selector != "object.ds-texture-layer")
            .collect();
        assert!(offences.is_empty(), "{}: {offences:#?}", case.name);
    }
}
