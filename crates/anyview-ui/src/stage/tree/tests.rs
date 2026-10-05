use super::*;
use crate::stage::row::RowNo;
use anyview_core::{OpenNodes, TreePath};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

fn at(steps: &[u32]) -> TreePath {
    steps.iter().fold(TreePath::root(), |p, s| p.child(*s))
}
fn browsing(open: OpenNodes) -> TreeStage {
    TreeStage::Browsing { open }
}
fn selected(open: OpenNodes, row: u32) -> TreeStage {
    TreeStage::Selected {
        open,
        row: RowNo(row),
    }
}
fn top() -> OpenNodes {
    OpenNodes::top_level()
}

/// Name, state before, input, state after.
type Case = (&'static str, TreeStage, TreeIn, TreeStage);

#[test]
fn the_tree_stage_steps_as_the_table_says() {
    let cases: Vec<Case> = vec![
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
        let (next, outs) = before.step(input, Stamp(0), &TreeParams, &());
        assert_eq!(next, after, "{name}");
        assert!(outs.is_empty(), "{name}");
        assert_eq!(next.wake(), None, "{name}: no timer");
    }
}
