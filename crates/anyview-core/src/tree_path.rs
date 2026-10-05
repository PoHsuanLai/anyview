//! Where a node of a JSON tree is, and which nodes are open: what the tree view's machine keeps
//! and the tree back end reads.

use std::collections::BTreeSet;

/// Where a node is: the position of each child to take, from the root. The root is the empty path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TreePath(Vec<u32>);

impl TreePath {
    /// The root.
    pub fn root() -> Self {
        TreePath(Vec::new())
    }

    /// The path to this node's child at `position`.
    pub fn child(&self, position: u32) -> Self {
        let mut steps = self.0.clone();
        steps.push(position);
        TreePath(steps)
    }

    /// The position of each step, from the root.
    pub fn steps(&self) -> &[u32] {
        &self.0
    }

    /// How many steps below the root the node is.
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    /// The position of this node among its parent's children, and the parent's path; `None` for
    /// the root.
    pub fn split_last(&self) -> Option<(u32, TreePath)> {
        let (last, parent) = self.0.split_last()?;
        Some((*last, TreePath(parent.to_vec())))
    }
}

/// The nodes that are open, so their children show. A node closed here is closed whatever its
/// descendants say: they are kept, and show again when it opens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenNodes(BTreeSet<TreePath>);

impl OpenNodes {
    /// Only the root open: the top level of a document shows and nothing below it.
    pub fn top_level() -> Self {
        OpenNodes(BTreeSet::from([TreePath::root()]))
    }

    /// Whether `path` is open.
    pub fn is_open(&self, path: &TreePath) -> bool {
        self.0.contains(path)
    }

    /// The same set with `path` closed when it was open and open when it was closed.
    pub fn toggled(mut self, path: &TreePath) -> Self {
        if !self.0.remove(path) {
            self.0.insert(path.clone());
        }
        self
    }

    /// The same set with `path` open.
    pub fn opened(mut self, path: TreePath) -> Self {
        self.0.insert(path);
        self
    }

    /// The same set with `path` closed.
    pub fn closed(mut self, path: &TreePath) -> Self {
        self.0.remove(path);
        self
    }

    /// The positions, ascending, of `parent`'s children that are open.
    pub fn open_children(&self, parent: &TreePath) -> Vec<u32> {
        let depth = parent.depth();
        self.0
            .range(parent.child(0)..)
            .take_while(|path| path.steps().starts_with(parent.steps()))
            .filter(|path| path.depth() == depth + 1)
            .filter_map(|path| path.steps().last().copied())
            .collect()
    }

    /// How many nodes are open.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no node is open.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(steps: &[u32]) -> TreePath {
        steps
            .iter()
            .fold(TreePath::root(), |at, step| at.child(*step))
    }

    #[test]
    fn toggling_opens_a_closed_node_and_closes_an_open_one() {
        let open = OpenNodes::top_level();
        let a = path(&[2]);
        let opened = open.clone().toggled(&a);
        assert!(opened.is_open(&a));
        assert_eq!(opened.len(), open.len() + 1);
        let closed = opened.toggled(&a);
        assert_eq!(closed, open);
    }

    #[test]
    fn the_open_children_of_a_node_are_its_direct_ones_in_order() {
        let open = [
            path(&[]),
            path(&[5]),
            path(&[1]),
            path(&[1, 3]),
            path(&[1, 3, 0]),
            path(&[1, 2]),
            path(&[10]),
        ]
        .into_iter()
        .fold(OpenNodes::default(), |set, node| set.opened(node));
        // name, parent, wanted
        const CASES: &[(&str, &[u32], &[u32])] = &[
            ("root", &[], &[1, 5, 10]),
            ("a child with two open", &[1], &[2, 3]),
            ("a leaf level", &[1, 3], &[0]),
            ("nothing open below", &[5], &[]),
            ("a path that is not open", &[7], &[]),
        ];
        for (name, parent, want) in CASES {
            assert_eq!(open.open_children(&path(parent)), *want, "{name}");
        }
    }

    #[test]
    fn a_path_knows_its_parent_and_its_depth() {
        assert_eq!(path(&[]).split_last(), None);
        assert_eq!(path(&[4, 7]).split_last(), Some((7, path(&[4]))));
        assert_eq!(path(&[4, 7]).depth(), 2);
    }
}
