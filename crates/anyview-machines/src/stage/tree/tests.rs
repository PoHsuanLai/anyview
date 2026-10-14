use super::*;
use crate::stage::find::{FindHits, FindOut, HitCount, HitIndex, StageFind};
use crate::stage::row::{RowNo, RowStep};
use crate::stage::zoom::{ContentScale, ZoomDir};
use crate::typed::TypedText;
use anyview_core::{OpenNodes, TreePath};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

fn at(steps: &[u32]) -> TreePath {
    steps.iter().fold(TreePath::root(), |p, s| p.child(*s))
}
fn browsing(open: OpenNodes) -> TreeStage {
    TreeStage::Browsing {
        open,
        find: None,
        scale: ContentScale::ACTUAL,
    }
}
fn selected(open: OpenNodes, row: u32) -> TreeStage {
    TreeStage::Selected {
        open,
        row: RowNo(row),
        find: None,
        scale: ContentScale::ACTUAL,
    }
}
fn top() -> OpenNodes {
    OpenNodes::top_level()
}

/// Ten visible rows, four to a page.
const PARAMS: TreeParams = TreeParams { rows: 10, page: 4 };

/// Name, state before, input, state after.
type Case = (&'static str, TreeStage, TreeIn, TreeStage);

#[test]
fn the_tree_stage_steps_as_the_table_says() {
    let cases: Vec<Case> = vec![
        (
            "down with no cursor picks the first row",
            browsing(top()),
            TreeIn::Move(RowStep::Down),
            selected(top(), 0),
        ),
        (
            "page down moves by the room",
            selected(top(), 1),
            TreeIn::Move(RowStep::PageDown),
            selected(top(), 5),
        ),
        (
            "end goes to the last visible row",
            selected(top(), 1),
            TreeIn::Move(RowStep::Bottom),
            selected(top(), 9),
        ),
        (
            "a file opens with its top level open",
            TreeStage::default(),
            TreeIn::Elapsed,
            browsing(top()),
        ),
        (
            "toggling a closed node opens it",
            browsing(top()),
            TreeIn::Toggle(at(&[1])),
            browsing(top().opened(at(&[1]))),
        ),
        (
            "toggling an open node closes it",
            browsing(top().opened(at(&[1]))),
            TreeIn::Toggle(at(&[1])),
            browsing(top()),
        ),
        (
            "closing the root hides the top level",
            browsing(top()),
            TreeIn::Close(TreePath::root()),
            browsing(OpenNodes::default()),
        ),
        (
            "opening an open node changes nothing",
            browsing(top()),
            TreeIn::Open(TreePath::root()),
            browsing(top()),
        ),
        (
            "a closed parent keeps what was open under it",
            browsing(top().opened(at(&[1])).opened(at(&[1, 2]))),
            TreeIn::Close(at(&[1])),
            browsing(top().opened(at(&[1, 2]))),
        ),
        (
            "collapse all goes back to the top level",
            browsing(top().opened(at(&[1])).opened(at(&[3, 0]))),
            TreeIn::CollapseAll,
            browsing(top()),
        ),
        (
            "selecting a row",
            browsing(top()),
            TreeIn::Select(RowNo(3)),
            selected(top(), 3),
        ),
        (
            "toggling keeps the cursor on its row",
            selected(top(), 3),
            TreeIn::Toggle(at(&[1])),
            selected(top().opened(at(&[1])), 3),
        ),
        (
            "collapse all drops the cursor",
            selected(top().opened(at(&[1])), 7),
            TreeIn::CollapseAll,
            browsing(top()),
        ),
        (
            "deselecting",
            selected(top(), 3),
            TreeIn::Deselect,
            browsing(top()),
        ),
        (
            "the clock changes nothing",
            selected(top(), 3),
            TreeIn::Elapsed,
            selected(top(), 3),
        ),
    ];
    for (name, before, input, after) in cases {
        let (next, outs) = before.step(input, Stamp(0), &PARAMS, &());
        assert_eq!(next, after, "{name}");
        assert!(outs.is_empty(), "{name}");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}

const CAT: TypedText = TypedText::from_static("cat");

fn finding(open: OpenNodes, row: Option<u32>, hits: FindHits) -> TreeStage {
    let find = Some(StageFind { query: CAT, hits });
    match row {
        Some(row) => TreeStage::Selected {
            open,
            row: RowNo(row),
            find,
            scale: ContentScale::ACTUAL,
        },
        None => TreeStage::Browsing {
            open,
            find,
            scale: ContentScale::ACTUAL,
        },
    }
}

fn found(count: u32, at: u32) -> FindHits {
    FindHits::answered(HitCount(count), HitIndex(at))
}

fn search(find: FindOut) -> Vec<TreeOut> {
    vec![TreeOut::Find(find)]
}

#[test]
fn a_find_in_a_tree_asks_for_a_search_follows_its_answer_and_opens_the_way_to_a_hit() {
    // name, state before, input, state after, outputs
    type FindCase = (&'static str, TreeStage, TreeIn, TreeStage, Vec<TreeOut>);
    let cases: Vec<FindCase> = vec![
        (
            "find asks for a search",
            browsing(top()),
            TreeIn::Find(CAT),
            finding(top(), None, FindHits::Pending),
            search(FindOut::Search(CAT)),
        ),
        (
            "the answer shows the nearest hit",
            finding(top(), None, FindHits::Pending),
            TreeIn::Results {
                query: CAT,
                count: HitCount(4),
                nearest: HitIndex(0),
            },
            finding(top(), None, found(4, 0)),
            search(FindOut::ShowHit(HitIndex(0))),
        ),
        (
            "the answer to an earlier text is dropped",
            finding(top(), None, FindHits::Pending),
            TreeIn::Results {
                query: TypedText::from_static("dog"),
                count: HitCount(4),
                nearest: HitIndex(0),
            },
            finding(top(), None, FindHits::Pending),
            vec![],
        ),
        (
            "next hit wraps past the last",
            finding(top(), Some(2), found(4, 3)),
            TreeIn::NextHit,
            finding(top(), Some(2), found(4, 0)),
            search(FindOut::ShowHit(HitIndex(0))),
        ),
        (
            "previous hit wraps past the first",
            finding(top(), None, found(4, 0)),
            TreeIn::PreviousHit,
            finding(top(), None, found(4, 3)),
            search(FindOut::ShowHit(HitIndex(3))),
        ),
        (
            "going to a hit shows it",
            finding(top(), None, found(4, 0)),
            TreeIn::GoToHit(HitIndex(2)),
            finding(top(), None, found(4, 2)),
            search(FindOut::ShowHit(HitIndex(2))),
        ),
        (
            "showing a hit opens the nodes above it and puts the cursor on it",
            finding(top(), None, found(4, 2)),
            TreeIn::Show {
                opened: vec![at(&[1]), at(&[1, 0])],
                row: RowNo(6),
            },
            finding(
                top().opened(at(&[1])).opened(at(&[1, 0])),
                Some(6),
                found(4, 2),
            ),
            vec![],
        ),
        (
            "closing the find puts the marks away and keeps the cursor",
            finding(top(), Some(2), found(4, 2)),
            TreeIn::CloseFind,
            selected(top(), 2),
            search(FindOut::Clear),
        ),
        (
            "collapse all keeps the find",
            finding(top().opened(at(&[1])), Some(7), found(4, 2)),
            TreeIn::CollapseAll,
            finding(top(), None, found(4, 2)),
            vec![],
        ),
    ];
    for (name, before, input, after, want) in cases {
        let (next, outs) = before.step(input, Stamp(0), &PARAMS, &());
        assert_eq!(next, after, "{name}: state");
        assert_eq!(outs, want, "{name}: outputs");
    }
}

#[test]
fn zooming_a_tree_steps_its_scale_and_keeps_where_it_is() {
    let bigger = ContentScale::ACTUAL.stepped(ZoomDir::In);
    let with = |stage: TreeStage, scale: ContentScale| match stage {
        TreeStage::Browsing { open, find, .. } => TreeStage::Browsing { open, find, scale },
        TreeStage::Selected {
            open, row, find, ..
        } => TreeStage::Selected {
            open,
            row,
            find,
            scale,
        },
    };
    let (zoomed, outs) =
        selected(top(), 3).step(TreeIn::ZoomStep(ZoomDir::In), Stamp(0), &PARAMS, &());
    assert_eq!(zoomed, with(selected(top(), 3), bigger));
    assert!(outs.is_empty());
    let (actual, _) = zoomed.step(TreeIn::ZoomActual, Stamp(0), &PARAMS, &());
    assert_eq!(actual, selected(top(), 3));
}
