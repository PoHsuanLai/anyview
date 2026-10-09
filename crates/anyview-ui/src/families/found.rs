//! The capsule's part of a find: finding is the palette's, so while a find is up and the palette
//! is closed the capsule says where the reader is among the hits and steps through them (⌘G and
//! ⇧⌘G do the same).

use crate::{Command, FindHits, Stage, StageCommand};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;
use ds::prelude::Icon;

/// Where a search stands in words, as `3 of 17`; nothing before it has found a hit.
pub(crate) fn readout(hits: FindHits) -> Option<String> {
    match (hits.current(), hits.count()) {
        (Some(current), Some(count)) => Some(format!("{} of {}", current.0 + 1, count.0)),
        (None, _) | (_, None) => None,
    }
}

/// The slots to append to a stage's capsule while its find has hits: a rule, the steps and the
/// readout. None while no find is up.
pub(crate) fn standing(stage: &Stage) -> Vec<RankedSlot<Command>> {
    let Some(text) = stage.find_state().and_then(|(_, hits)| readout(hits)) else {
        return Vec::new();
    };
    vec![
        CapsuleSlot::Divider.essential(),
        CapsuleSlot::button(
            Command::Stage(StageCommand::FindPrevious),
            "Previous Match",
            Icon::ChevronUp,
        )
        .essential(),
        CapsuleSlot::Readout(text).essential(),
        CapsuleSlot::button(
            Command::Stage(StageCommand::FindNext),
            "Next Match",
            Icon::ChevronDown,
        )
        .essential(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HitCount, HitIndex};

    #[test]
    fn the_readout_counts_from_one_and_waits_for_a_hit() {
        // name, where the search stands, the words
        const CASES: &[(&str, FindHits, Option<&str>)] = &[
            ("nothing typed", FindHits::Idle, None),
            ("waiting", FindHits::Pending, None),
            ("nothing found", FindHits::NoMatch, None),
            (
                "the third of seventeen",
                FindHits::answered(HitCount(17), HitIndex(2)),
                Some("3 of 17"),
            ),
        ];
        for (name, hits, want) in CASES {
            assert_eq!(readout(*hits).as_deref(), *want, "{name}");
        }
    }
}
