//! A find in a tree: the keys and values, among the nodes of a document, that hold the phrase,
//! found on a worker by walking the document in reading order (a node's children are read from the
//! file as the walk reaches them, so a large document is never held), the words around each for the
//! palette's list, and what opens to show one.

use super::doc::TreeDoc;
use crate::families::places::{NodeHit, NodePart};
use crate::families::{FoundHits, HitLine, Snippet, clip};
use crate::{HitIndex, RowNo, Stage, StageIn, TreeIn, TypedText};
use anyview_core::work::{Stop, StopState};
use anyview_core::{FindOptions, LineIndex, TreePath};
use anyview_text::{MAX_HITS, Needle, NodeKind, RowLabel, Tree, TreeRow};

/// How many children one read takes.
const BATCH: u32 = 512;

/// How many hits keep the words around them, for the palette's list.
const SNIPPETS: usize = 200;

/// What a row is called, as it reads.
fn label_of(label: &RowLabel) -> String {
    match label {
        RowLabel::Root => "(root)".to_owned(),
        RowLabel::Key(key) => key.clone(),
        RowLabel::Index(index) => index.to_string(),
    }
}

/// Whether a node of this kind shows a value of its own (a container shows only its size; a
/// JSON Lines line that is not a value shows the line as written, which a find reads too).
fn is_scalar(kind: NodeKind) -> bool {
    match kind {
        NodeKind::String
        | NodeKind::Number
        | NodeKind::True
        | NodeKind::False
        | NodeKind::Null
        | NodeKind::Unreadable => true,
        NodeKind::Object | NodeKind::Array => false,
    }
}

/// The hits so far, and the words around the first of them.
#[derive(Default)]
struct Found {
    hits: Vec<NodeHit>,
    snippets: Vec<Snippet>,
}

impl Found {
    /// A hit of `part` of the row `label: preview` at `path`, matching the bytes `from..to` of that
    /// part's text.
    fn add(&mut self, at: (&TreePath, &str), part: NodePart, row: (&str, &str), span: (u32, u32)) {
        let (path, trail) = at;
        let (label, preview) = row;
        if self.snippets.len() < SNIPPETS {
            let line = format!("{label}: {preview}");
            let shift = match part {
                NodePart::Key => 0,
                NodePart::Value => label.len() + 2,
            };
            // a u32 fits a usize
            self.snippets.push(clip(
                &line,
                shift + span.0 as usize,
                shift + span.1 as usize,
            ));
        }
        self.hits.push(NodeHit {
            path: path.clone(),
            part,
            from: span.0,
            to: span.1,
            trail: trail.to_owned(),
        });
    }
}

/// Look in the row at `path` (under the keys of `trail`) for the phrase: in its key, and in its
/// value when it has one.
fn look(needle: &Needle, row: &TreeRow, at: (&TreePath, &str), found: &mut Found) {
    let label = label_of(&row.label);
    if let RowLabel::Key(key) = &row.label {
        for hit in needle.hits_in(LineIndex(0), key) {
            if found.hits.len() < MAX_HITS {
                found.add(
                    at,
                    NodePart::Key,
                    (&label, &row.preview),
                    (hit.from.0, hit.to.0),
                );
            }
        }
    }
    if is_scalar(row.kind) {
        for hit in needle.hits_in(LineIndex(0), &row.preview) {
            if found.hits.len() < MAX_HITS {
                found.add(
                    at,
                    NodePart::Value,
                    (&label, &row.preview),
                    (hit.from.0, hit.to.0),
                );
            }
        }
    }
}

/// A container the walk is in: which of its children is next, and the window of them read.
struct Frame {
    path: TreePath,
    /// The keys above its children, as the palette says where a hit is.
    trail: String,
    len: u32,
    next: u32,
    start: u32,
    window: Vec<TreeRow>,
}

/// The child the walk reaches: its row, where it is, and the keys above it.
struct Next {
    row: TreeRow,
    path: TreePath,
    trail: String,
}

impl Frame {
    fn new(path: TreePath, trail: String, len: u32) -> Frame {
        Frame {
            path,
            trail,
            len,
            next: 0,
            start: 0,
            window: Vec::new(),
        }
    }

    /// The next child, reading a window of them when the one held is used up; `None` at the end.
    fn take_next(&mut self, tree: &Tree) -> Option<Next> {
        if self.next >= self.len {
            return None;
        }
        let index = self.next;
        let held = index
            .checked_sub(self.start)
            .and_then(|at| self.window.get(at as usize)); // a u32 fits a usize
        let row = match held {
            Some(row) => row.clone(),
            None => {
                self.start = index;
                self.window = tree
                    .children(&self.path, index..index.saturating_add(BATCH).min(self.len))
                    .unwrap_or_default();
                self.window.first()?.clone()
            }
        };
        self.next += 1;
        Some(Next {
            row,
            path: self.path.child(index),
            trail: self.trail.clone(),
        })
    }
}

/// `trail` with `label` after it.
fn below(trail: &str, label: &str) -> String {
    if trail.is_empty() {
        label.to_owned()
    } else {
        format!("{trail} \u{203a} {label}")
    }
}

/// Every key and value of `doc` in which `query` occurs as `options` say, in document order, at
/// most [`MAX_HITS`] of them: blocking, walks the document once. None for an empty query; the
/// hits found so far when `stop` is raised.
pub(crate) fn search_tree(
    doc: &TreeDoc,
    query: &TypedText,
    options: FindOptions,
    stop: &Stop,
) -> FoundHits {
    let Some(needle) = Needle::new(query.as_str()).map(|needle| needle.with_options(options))
    else {
        return FoundHits::default();
    };
    let tree = &doc.tree;
    let root_path = TreePath::root();
    let Ok(root) = tree.row(&root_path) else {
        return FoundHits::default();
    };
    let mut found = Found::default();
    look(&needle, &root, (&root_path, ""), &mut found);
    let mut stack = vec![Frame::new(root_path, String::new(), root.children.0)];
    while found.hits.len() < MAX_HITS && stop.stopped() == StopState::Running {
        let Some(frame) = stack.last_mut() else {
            break;
        };
        let Some(Next { row, path, trail }) = frame.take_next(tree) else {
            stack.pop();
            continue;
        };
        look(&needle, &row, (&path, &trail), &mut found);
        if row.children.0 > 0 {
            let inside = below(&trail, &label_of(&row.label));
            stack.push(Frame::new(path, inside, row.children.0));
        }
    }
    FoundHits::nodes(found.hits).with_snippets(found.snippets)
}

/// The rows of the palette's list: each hit by the words around it, and the keys above it.
pub(super) fn lines(found: &FoundHits, upto: u32) -> Vec<HitLine> {
    (0..upto.min(found.count().0))
        .map(|index| {
            let snippet = found.snippet(HitIndex(index)).cloned().unwrap_or_default();
            let place = found
                .node(HitIndex(index))
                .map(|hit| hit.trail.clone())
                .unwrap_or_default();
            HitLine {
                context: snippet.text,
                matched: snippet.matched,
                place,
            }
        })
        .collect()
}

/// The inputs that show hit `hit`: the nodes above it open, and the cursor goes to its row.
pub(super) fn show(doc: &TreeDoc, stage: &Stage, found: &FoundHits, hit: HitIndex) -> Vec<StageIn> {
    let (Stage::Tree(tree), Some(node)) = (stage, found.node(hit)) else {
        return Vec::new();
    };
    let steps = node.path.steps();
    let mut open = tree.open().clone();
    let mut opened = Vec::new();
    for depth in 0..steps.len() {
        let above = steps[..depth]
            .iter()
            .fold(TreePath::root(), |path, step| path.child(*step));
        if !open.is_open(&above) {
            open = open.opened(above.clone());
            opened.push(above);
        }
    }
    match doc.tree.visible_index(&open, &node.path) {
        Some(row) => vec![StageIn::Tree(TreeIn::Show {
            opened,
            row: RowNo(row),
        })],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{Facts, OpenNodes, TreeFormat};
    use anyview_text::Coverage;

    const DOC: &str =
        r#"{"city":"Lima","people":[{"name":"Ana","home":"Lima"},{"name":"Luis"}],"count":3}"#;

    fn doc() -> TreeDoc {
        TreeDoc {
            tree: Tree::parse(DOC, TreeFormat::Json).unwrap(),
            coverage: Coverage::Whole,
            facts: Facts::empty(),
        }
    }

    fn search(query: &str) -> FoundHits {
        search_tree(
            &doc(),
            &TypedText::new(query),
            FindOptions::default(),
            &Stop::new(),
        )
    }

    fn at(steps: &[u32]) -> TreePath {
        steps
            .iter()
            .fold(TreePath::root(), |path, step| path.child(*step))
    }

    #[test]
    fn a_search_finds_keys_and_values_at_any_depth_in_document_order() {
        // name, phrase, the nodes hit and the part of each
        type Case = (&'static str, &'static str, Vec<(Vec<u32>, NodePart)>);
        let cases: Vec<Case> = vec![
            (
                "a value, twice",
                "lima",
                vec![(vec![0], NodePart::Value), (vec![1, 0, 1], NodePart::Value)],
            ),
            (
                "a key at the top and a key deep down",
                "name",
                vec![
                    (vec![1, 0, 0], NodePart::Key),
                    (vec![1, 1, 0], NodePart::Key),
                ],
            ),
            ("a number", "3", vec![(vec![2], NodePart::Value)]),
            ("a container's size is not a value", "keys", vec![]),
            ("nothing", "zzz", vec![]),
            ("an empty phrase", "", vec![]),
        ];
        for (name, phrase, want) in cases {
            let found = search(phrase);
            let got: Vec<(Vec<u32>, NodePart)> = (0..found.count().0)
                .map(|at| {
                    let hit = found.node(HitIndex(at)).unwrap();
                    (hit.path.steps().to_vec(), hit.part)
                })
                .collect();
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn a_hit_says_where_it_is_by_the_keys_above_it_and_marks_itself_in_its_words() {
        let found = search("lima");
        let deep = found.node(HitIndex(1)).unwrap();
        assert_eq!(deep.trail, "people \u{203a} 0");
        assert_eq!(found.node(HitIndex(0)).unwrap().trail, "");
        let snippet = found.snippet(HitIndex(1)).unwrap();
        assert_eq!(snippet.text, "home: \"Lima\"");
        assert_eq!(&snippet.text[snippet.matched.clone()], "Lima");
    }

    #[test]
    fn showing_a_hit_opens_the_nodes_above_it_and_puts_the_cursor_on_its_row() {
        let found = search("luis");
        let closed = Stage::Tree(crate::TreeStage::default());
        // the root is open: people, then people[1] and its name's value are closed away
        let shown = show(&doc(), &closed, &found, HitIndex(0));
        let StageIn::Tree(TreeIn::Show { opened, row }) = &shown[0] else {
            panic!("a hit in a closed node shows by opening the way to it");
        };
        assert_eq!(opened, &[at(&[1]), at(&[1, 1])]);
        // root, city, people, [0], [1], name, count with people and people[1] open: row 5
        let open = OpenNodes::top_level().opened(at(&[1])).opened(at(&[1, 1]));
        assert_eq!(
            doc().tree.visible_index(&open, &at(&[1, 1, 0])),
            Some(row.0)
        );
        // a node that is already open is not opened again
        let open_already = Stage::Tree(crate::TreeStage::Browsing {
            open,
            find: None,
            scale: crate::ContentScale::ACTUAL,
        });
        let again = show(&doc(), &open_already, &found, HitIndex(0));
        assert!(
            matches!(&again[0], StageIn::Tree(TreeIn::Show { opened, .. }) if opened.is_empty())
        );
    }
}
