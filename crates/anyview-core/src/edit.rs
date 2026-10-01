//! Edits: each change the viewer can save in place, described as data.

use crate::units::{Axis, PageIndex, PageRange, QuarterTurn};
use ds_core::word::Word;

/// One change to a file. An edit is a value: the save pipeline applies it, undo stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Edit {
    /// Turn the content clockwise by this much.
    Rotate(QuarterTurn),
    /// Mirror the content across an axis.
    Flip(Axis),
    /// Remove a run of pages.
    DeletePages(PageRange),
    /// Move one page to a new place.
    MovePage {
        /// The page's place now.
        from: PageIndex,
        /// The place it moves to.
        to: PageIndex,
    },
}

/// The sorts of edit, without their options: what a kind of file offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Word)]
#[serde(rename_all = "snake_case")]
#[word(case = snake)]
pub enum EditKind {
    /// [`Edit::Rotate`].
    Rotate,
    /// [`Edit::Flip`].
    Flip,
    /// [`Edit::DeletePages`].
    DeletePages,
    /// [`Edit::MovePage`].
    MovePage,
}

impl Edit {
    /// Which sort of edit this is.
    pub fn kind(&self) -> EditKind {
        match self {
            Edit::Rotate(_) => EditKind::Rotate,
            Edit::Flip(_) => EditKind::Flip,
            Edit::DeletePages(_) => EditKind::DeletePages,
            Edit::MovePage { .. } => EditKind::MovePage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds_core::testing::word_matches_serde;

    fn range(first: u32, last: u32) -> PageRange {
        PageRange::new(PageIndex(first), PageIndex(last)).unwrap()
    }

    #[test]
    fn each_edit_reports_its_kind() {
        let cases = [
            (Edit::Rotate(QuarterTurn::Quarter), EditKind::Rotate),
            (Edit::Flip(Axis::Vertical), EditKind::Flip),
            (Edit::DeletePages(range(1, 2)), EditKind::DeletePages),
            (
                Edit::MovePage {
                    from: PageIndex(0),
                    to: PageIndex(3),
                },
                EditKind::MovePage,
            ),
        ];
        for (edit, kind) in cases {
            assert_eq!(edit.kind(), kind, "{edit:?}");
        }
    }

    #[test]
    fn edits_round_trip_adjacently_tagged() {
        let cases = [
            (
                Edit::Rotate(QuarterTurn::Half),
                r#"{"kind":"rotate","v":"half"}"#,
            ),
            (
                Edit::Flip(Axis::Horizontal),
                r#"{"kind":"flip","v":"horizontal"}"#,
            ),
            (
                Edit::DeletePages(range(2, 4)),
                r#"{"kind":"delete_pages","v":{"first":2,"last":4}}"#,
            ),
            (
                Edit::MovePage {
                    from: PageIndex(1),
                    to: PageIndex(5),
                },
                r#"{"kind":"move_page","v":{"from":1,"to":5}}"#,
            ),
        ];
        for (edit, json) in cases {
            assert_eq!(serde_json::to_string(&edit).unwrap(), json, "{edit:?}");
            assert_eq!(
                serde_json::from_str::<Edit>(json).unwrap(),
                edit,
                "{edit:?}"
            );
        }
    }

    #[test]
    fn edit_kinds_are_stored_as_their_slugs() {
        word_matches_serde::<EditKind>();
    }
}
