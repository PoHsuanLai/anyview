//! The rows a tree shows when some of its nodes are open: the document flattened in reading
//! order, read one window at a time so a million-item array costs the rows on screen.

use super::node::Node;
use super::rows::{self, RowLabel, TreeRow};
use super::{Tree, TreePath};
use anyview_core::OpenNodes;
use std::ops::Range;

/// Whether a visible node opens, and whether it is open now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Openness {
    /// A scalar or an empty container: nothing to open.
    Leaf,
    /// A container that is closed.
    Closed,
    /// A container that is open, its children following it.
    Open,
}

/// One row of the flattened tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    /// Where the node is.
    pub path: TreePath,
    /// How many levels below the root it is; the root is 0.
    pub depth: u32,
    /// Whether it opens.
    pub openness: Openness,
    /// What it shows.
    pub row: TreeRow,
}

fn children_of(node: &Node) -> Option<usize> {
    match node {
        Node::Object(entries) => Some(entries.len()),
        Node::Array(items) => Some(items.len()),
        Node::Scalar { .. } => None,
    }
}

fn child(node: &Node, at: usize) -> Option<(RowLabel, &Node)> {
    match node {
        Node::Object(entries) => entries
            .get(at)
            .map(|(key, node)| (RowLabel::Key(key.clone()), node)),
        Node::Array(items) => items
            .get(at)
            .map(|node| (RowLabel::Index(u32::try_from(at).unwrap_or(u32::MAX)), node)),
        Node::Scalar { .. } => None,
    }
}

/// How many rows `node` and its open descendants make.
fn count(node: &Node, path: &TreePath, open: &OpenNodes) -> u64 {
    let Some(len) = children_of(node) else {
        return 1;
    };
    if !open.is_open(path) {
        return 1;
    }
    let opened: Vec<u32> = open
        .open_children(path)
        .into_iter()
        .filter(|at| (*at as usize) < len) // a u32 fits a usize
        .collect();
    let nested: u64 = opened
        .iter()
        .filter_map(|at| child(node, *at as usize).map(|(_, node)| (at, node)))
        .map(|(at, node)| count(node, &path.child(*at), open))
        .sum();
    1 + (len - opened.len()) as u64 + nested // a usize fits a u64
}

/// The window being cut: rows still to skip, rows still wanted, and the rows collected.
struct Cut {
    skip: u64,
    take: u64,
    rows: Vec<VisibleRow>,
}

impl Cut {
    fn full(&self) -> bool {
        self.take == 0
    }

    fn push(&mut self, make: impl FnOnce() -> VisibleRow) {
        if self.skip > 0 {
            self.skip -= 1;
        } else if self.take > 0 {
            self.take -= 1;
            self.rows.push(make());
        }
    }
}

fn openness(node: &Node, path: &TreePath, open: &OpenNodes) -> Openness {
    match children_of(node) {
        None | Some(0) => Openness::Leaf,
        Some(_) if open.is_open(path) => Openness::Open,
        Some(_) => Openness::Closed,
    }
}

/// Collect the rows of `node` into `cut`: itself, then its open children.
fn walk(node: &Node, label: RowLabel, path: &TreePath, open: &OpenNodes, cut: &mut Cut) {
    let depth = u32::try_from(path.depth()).unwrap_or(u32::MAX);
    cut.push(|| VisibleRow {
        path: path.clone(),
        depth,
        openness: openness(node, path, open),
        row: rows::row(label, node),
    });
    let Some(len) = children_of(node).filter(|_| open.is_open(path)) else {
        return;
    };
    let mut at = 0usize;
    let opened = open.open_children(path);
    let plain = |from: usize, to: usize, cut: &mut Cut| {
        // The children from..to are closed or leaves: one row each, so most are skipped by sum.
        let run = (to - from) as u64; // a usize fits a u64
        if cut.skip >= run {
            cut.skip -= run;
            return;
        }
        let first = from + cut.skip as usize; // skip < run, a usize
        cut.skip = 0;
        for position in first..to {
            if cut.full() {
                return;
            }
            if let Some((label, node)) = child(node, position) {
                let at_path = path.child(u32::try_from(position).unwrap_or(u32::MAX));
                cut.push(|| VisibleRow {
                    depth: depth + 1,
                    openness: openness(node, &at_path, open),
                    path: at_path.clone(),
                    row: rows::row(label, node),
                });
            }
        }
    };
    for position in opened.into_iter().map(|p| p as usize).filter(|p| *p < len) {
        plain(at, position, cut);
        if cut.full() {
            return;
        }
        if let Some((label, node)) = child(node, position) {
            let at_path = path.child(u32::try_from(position).unwrap_or(u32::MAX));
            // A skip that covers the whole subtree passes over it without building rows.
            let size = count(node, &at_path, open);
            if cut.skip >= size {
                cut.skip -= size;
            } else {
                walk(node, label, &at_path, open, cut);
            }
        }
        at = position + 1;
    }
    plain(at, len, cut);
}

impl Tree {
    /// How many rows the tree shows with `open` nodes open.
    pub fn visible_count(&self, open: &OpenNodes) -> u32 {
        u32::try_from(count(&self.root, &TreePath::root(), open)).unwrap_or(u32::MAX)
    }

    /// The rows in `window` of the flattened tree, cut to the rows that exist.
    pub fn visible(&self, open: &OpenNodes, window: Range<u32>) -> Vec<VisibleRow> {
        let mut cut = Cut {
            skip: u64::from(window.start),
            take: u64::from(window.end.saturating_sub(window.start)),
            rows: Vec::new(),
        };
        walk(
            &self.root,
            RowLabel::Root,
            &TreePath::root(),
            open,
            &mut cut,
        );
        cut.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::TreeFormat;

    const DOC: &str = r#"{"a":[10,20,{"x":1}],"b":"s","c":{"d":null,"e":[]}}"#;

    fn tree() -> Tree {
        Tree::parse(DOC, TreeFormat::Json).unwrap()
    }

    fn labels(rows: &[VisibleRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let key = match &r.row.label {
                    RowLabel::Root => "root".to_owned(),
                    RowLabel::Key(k) => k.clone(),
                    RowLabel::Index(i) => format!("[{i}]"),
                };
                format!("{}{}", "-".repeat(r.depth as usize), key)
            })
            .collect()
    }

    fn p(steps: &[u32]) -> TreePath {
        steps.iter().fold(TreePath::root(), |at, s| at.child(*s))
    }

    #[test]
    fn the_flattened_tree_follows_what_is_open() {
        let top = OpenNodes::top_level();
        let a = top.clone().opened(p(&[0]));
        let deep = a.clone().opened(p(&[0, 2])).opened(p(&[2]));
        // name, open set, rows
        let cases: Vec<(&str, OpenNodes, Vec<&str>)> = vec![
            ("nothing open", OpenNodes::default(), vec!["root"]),
            ("top level", top.clone(), vec!["root", "-a", "-b", "-c"]),
            (
                "one child open",
                a,
                vec!["root", "-a", "--[0]", "--[1]", "--[2]", "-b", "-c"],
            ),
            (
                "nested open",
                deep,
                vec![
                    "root", "-a", "--[0]", "--[1]", "--[2]", "---x", "-b", "-c", "--d", "--e",
                ],
            ),
        ];
        for (name, open, want) in cases {
            let rows = tree().visible(&open, 0..u32::MAX);
            assert_eq!(labels(&rows), want, "{name}");
            assert_eq!(
                tree().visible_count(&open) as usize,
                want.len(),
                "{name}: count"
            );
        }
    }

    #[test]
    fn a_window_is_the_same_rows_as_the_whole_cut_out() {
        let open = OpenNodes::top_level()
            .opened(p(&[0]))
            .opened(p(&[0, 2]))
            .opened(p(&[2]));
        let whole = tree().visible(&open, 0..u32::MAX);
        for start in 0..whole.len() as u32 + 2 {
            for len in 0..5u32 {
                let got = tree().visible(&open, start..start + len);
                let want: Vec<_> = whole
                    .iter()
                    .skip(start as usize)
                    .take(len as usize)
                    .cloned()
                    .collect();
                assert_eq!(got, want, "window {start}+{len}");
            }
        }
    }

    #[test]
    fn a_big_array_is_read_by_its_window_alone() {
        let text = format!(
            "[{}]",
            (0..200_000)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let big = Tree::parse(&text, TreeFormat::Json).unwrap();
        let open = OpenNodes::top_level();
        assert_eq!(big.visible_count(&open), 200_001);
        let rows = big.visible(&open, 150_000..150_003);
        assert_eq!(labels(&rows), vec!["-[149999]", "-[150000]", "-[150001]"]);
        assert_eq!(rows[0].openness, Openness::Leaf);
    }
}
