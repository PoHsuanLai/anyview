//! The views' markup: each component rendered through dioxus-ssr inside a `Ds` root, compared with
//! a golden under `tests/snapshots/views/` and linted against quire's stylesheet and the viewer's.
//! `DS_BLESS=1 cargo test -p anyview-ui views::` rewrites the goldens.

use super::chrome::{Controls, Titlebar};
use super::context::items_of;
use super::panel::InfoPanel;
use crate::testing::golden;
use crate::{Command, ContextPick, PanelTab, PanelTabs, StageCommand};
use anyview_core::{FactLabel, FactValue, Facts, FileAction, FormatKind};
use anyview_machines::seam::entries;
use dioxus::prelude::*;
use ds::assembly::ds::Inject;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::{RankedSlot, essentials};
use ds::host::measure::Anchor;
use ds::prelude::*;
use ds_lint::{LintConfig, markup};

/// One component in one state.
struct Case {
    name: &'static str,
    make: fn() -> Element,
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

fn slots() -> Vec<RankedSlot<Command>> {
    essentials(vec![
        CapsuleSlot::button(
            Command::Stage(StageCommand::ZoomOut),
            "Zoom out",
            Icon::Minus,
        ),
        CapsuleSlot::Readout("100%".to_owned()),
        CapsuleSlot::button(Command::Stage(StageCommand::ZoomIn), "Zoom in", Icon::Plus),
        CapsuleSlot::Divider,
        CapsuleSlot::button(
            Command::File(FileAction::RotateLeft),
            "Rotate left",
            Icon::RotateLeft,
        ),
    ])
}

/// What a picture's palette lists, as the menu draws it. The golden draws the menu inline: a
/// floating menu is laid out on the window's overlay layer, which a markup render has none of.
fn picture_menu() -> Vec<crate::ContextEntry> {
    let listed: Vec<Command> = [
        FileAction::RotateLeft,
        FileAction::RotateRight,
        FileAction::CopyPath,
        FileAction::RevealInFolder,
        FileAction::Export,
        FileAction::Share,
        FileAction::Rename,
        FileAction::Duplicate,
        FileAction::MoveToTrash,
    ]
    .into_iter()
    .map(Command::File)
    .collect();
    entries(&listed, PanelTabs::of(&[PanelTab::Info]))
}

fn facts() -> Facts {
    Facts::empty()
        .with(FactLabel::Kind, FactValue::text("image/png"))
        .with(FactLabel::Lines, FactValue::text("120"))
}

/// A located photo as the Info tab lists it: every section but the document's, General last.
fn photo_facts() -> Facts {
    Facts::empty()
        .with(FactLabel::Dimensions, FactValue::text("4032 × 3024"))
        .with(FactLabel::Colour, FactValue::text("RGB, 8-bit"))
        .with(FactLabel::Resolution, FactValue::text("300 dpi"))
        .with(FactLabel::Camera, FactValue::text("Canon EOS R5"))
        .with(FactLabel::Exposure, FactValue::text("1/200 s · f/2.8"))
        .with(FactLabel::Taken, FactValue::text("1 May 2024 at 12:30"))
        .with(
            FactLabel::Coordinates,
            FactValue::text("37.7749° N, 122.4194° W"),
        )
        .with(FactLabel::Kind, FactValue::text("JPEG image"))
        .with(FactLabel::Size, FactValue::text("3.2 MB (3,214,880 bytes)"))
        .with(FactLabel::Modified, FactValue::text("2 Oct 2026 at 14:31"))
}

const CASES: &[Case] = &[
    Case {
        name: "titlebar-shown",
        make: || {
            root(rsx! {
                Titlebar { title: "quadrants.png", shown: Shown::Visible, onpointerenter: |()| {}, onpointerleave: |()| {} }
            })
        },
    },
    Case {
        name: "titlebar-hidden",
        make: || {
            root(rsx! {
                Titlebar { title: "quadrants.png", shown: Shown::Hidden, onpointerenter: |()| {}, onpointerleave: |()| {} }
            })
        },
    },
    Case {
        name: "controls-shown",
        make: || {
            root(rsx! {
                div { style: "position:relative; width:480px; height:200px",
                    Controls { slots: slots(), shown: Shown::Visible, onpick: |_| {}, onscrub: |_| {}, onlevel: |_| {}, onpointerenter: |()| {}, onpointerleave: |()| {} }
                }
            })
        },
    },
    Case {
        name: "context-menu-picture",
        make: || {
            root(rsx! {
                Menu::<ContextPick> {
                    placement: MenuPlacement::Context,
                    anchor: Anchor::Point(Point { x: Px(40.0), y: Px(30.0) }),
                    items: items_of(&picture_menu()),
                    flow: Flow::Inline,
                    onpick: |_| {},
                    onclose: |()| {},
                }
            })
        },
    },
    Case {
        name: "panel-info",
        make: || {
            root(rsx! {
                InfoPanel {
                    tab: PanelTab::Info,
                    tabs: PanelTabs::of(&[PanelTab::Info]),
                    name: "notes.txt".to_owned(),
                    kind: Some(FormatKind::PlainText),
                    facts: facts(),
                    body: None,
                    onchoose: |_| {},
                }
            })
        },
    },
    Case {
        name: "panel-info-photo",
        make: || {
            root(rsx! {
                InfoPanel {
                    tab: PanelTab::Info,
                    tabs: PanelTabs::of(&[PanelTab::Info]),
                    name: "IMG_0412.jpg".to_owned(),
                    kind: Some(FormatKind::Raster),
                    facts: photo_facts(),
                    body: None,
                    onchoose: |_| {},
                }
            })
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
fn every_view_matches_its_golden() {
    let failures: Vec<String> = CASES
        .iter()
        .filter_map(|case| golden::check(&format!("views/{}.html", case.name), &render(case)).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_view_lints_clean_against_both_stylesheets() {
    let css = format!("{}\n{}", ds::stylesheet(), super::stylesheet());
    for case in CASES {
        let offences = markup(&render(case), &css, &LintConfig::new(&ds::kits()));
        assert!(offences.is_empty(), "{}: {offences:#?}", case.name);
    }
}
