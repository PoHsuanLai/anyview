use super::*;
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

const PDF_TABS: PanelTabs =
    PanelTabs::of(&[PanelTab::Thumbnails, PanelTab::Contents, PanelTab::Info]);
const TEXT_TABS: PanelTabs = PanelTabs::of(&[PanelTab::Info]);
const MEDIA_TABS: PanelTabs = PanelTabs::of(&[PanelTab::Info, PanelTab::Tracks]);

const fn shown(tab: PanelTab) -> Panel {
    Panel::Shown { tab }
}

/// Name, tabs the file has, state before, input, state after, outputs.
type Case = (
    &'static str,
    PanelTabs,
    Panel,
    PanelIn,
    Panel,
    &'static [PanelOut],
);

const CASES: &[Case] = &[
    (
        "toggle opens on the first tab the format has",
        PDF_TABS,
        Panel::Hidden,
        PanelIn::Toggle,
        shown(PanelTab::Thumbnails),
        &[PanelOut::Show(PanelTab::Thumbnails)],
    ),
    (
        "toggle opens a text file on its only tab",
        TEXT_TABS,
        Panel::Hidden,
        PanelIn::Toggle,
        shown(PanelTab::Info),
        &[PanelOut::Show(PanelTab::Info)],
    ),
    (
        "toggle does nothing for a format with no panel",
        PanelTabs::NONE,
        Panel::Hidden,
        PanelIn::Toggle,
        Panel::Hidden,
        &[],
    ),
    (
        "choosing a tab opens the panel on it",
        MEDIA_TABS,
        Panel::Hidden,
        PanelIn::Choose(PanelTab::Tracks),
        shown(PanelTab::Tracks),
        &[PanelOut::Show(PanelTab::Tracks)],
    ),
    (
        "choosing a tab the format lacks is refused",
        TEXT_TABS,
        Panel::Hidden,
        PanelIn::Choose(PanelTab::Tracks),
        Panel::Hidden,
        &[],
    ),
    (
        "toggle closes a shown panel",
        PDF_TABS,
        shown(PanelTab::Contents),
        PanelIn::Toggle,
        Panel::Hidden,
        &[PanelOut::Hide],
    ),
    (
        "close closes a shown panel",
        PDF_TABS,
        shown(PanelTab::Info),
        PanelIn::Close,
        Panel::Hidden,
        &[PanelOut::Hide],
    ),
    (
        "close on a hidden panel is nothing",
        PDF_TABS,
        Panel::Hidden,
        PanelIn::Close,
        Panel::Hidden,
        &[],
    ),
    (
        "choosing another tab switches",
        PDF_TABS,
        shown(PanelTab::Thumbnails),
        PanelIn::Choose(PanelTab::Info),
        shown(PanelTab::Info),
        &[PanelOut::Show(PanelTab::Info)],
    ),
    (
        "choosing the shown tab is nothing",
        PDF_TABS,
        shown(PanelTab::Info),
        PanelIn::Choose(PanelTab::Info),
        shown(PanelTab::Info),
        &[],
    ),
    (
        "choosing a tab the format lacks keeps the shown one",
        TEXT_TABS,
        shown(PanelTab::Info),
        PanelIn::Choose(PanelTab::Contents),
        shown(PanelTab::Info),
        &[],
    ),
    (
        "a new file without the shown tab moves to its first",
        TEXT_TABS,
        shown(PanelTab::Thumbnails),
        PanelIn::TabsChanged,
        shown(PanelTab::Info),
        &[PanelOut::Show(PanelTab::Info)],
    ),
    (
        "a new file with no panel closes it",
        PanelTabs::NONE,
        shown(PanelTab::Thumbnails),
        PanelIn::TabsChanged,
        Panel::Hidden,
        &[PanelOut::Hide],
    ),
    (
        "a new file that still has the shown tab keeps it",
        PDF_TABS,
        shown(PanelTab::Contents),
        PanelIn::TabsChanged,
        shown(PanelTab::Contents),
        &[],
    ),
    (
        "the clock does nothing",
        PDF_TABS,
        shown(PanelTab::Contents),
        PanelIn::Elapsed,
        shown(PanelTab::Contents),
        &[],
    ),
];

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, tabs, from, input, state, outs) in CASES {
        let params = PanelParams { tabs: *tabs };
        let (next, out) = from.step(*input, Stamp(0), &params, &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: a panel keeps no timer");
    }
}

#[test]
fn a_tab_set_lists_its_members() {
    assert_eq!(PDF_TABS.first(), Some(PanelTab::Thumbnails));
    assert_eq!(PanelTabs::NONE.first(), None);
    assert!(MEDIA_TABS.contains(PanelTab::Tracks));
    assert!(!MEDIA_TABS.contains(PanelTab::Contents));
}
