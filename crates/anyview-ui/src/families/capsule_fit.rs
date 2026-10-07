//! What a capsule shows in the width it has. A capsule is as wide as its slots, and a stage can
//! be narrower than that (a window at its least width, a panel open beside the stage), so a
//! family ranks its slots and [`fit_slots`] drops the least important, a rank at a time, until
//! what is left fits: the way QuickTime and Preview thin their controls as the window narrows.
//! What goes is still reachable from the palette and the context menu.
//!
//! The widths are quire's capsule (`capsule.css`) read through its tokens: a button is a Large
//! control square, a gap is `--s-4`, the capsule pads `--s-8` each side and floats `--s-16` from
//! the stage's edges, a readout is at least `--s-36` wide, a divider is a hairline between
//! `--s-4` margins, the progress bar is at least 120 and the level is 96, and a capsule that
//! holds a progress bar is at most 640 wide.

use super::view::Area;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::style::tokens::control_size::ControlSize;

/// One step of the spacing scale, in pixels: `--s-4`.
const S4: u32 = 4;
/// `--s-8`.
const S8: u32 = 8;
/// `--s-16`.
const S16: u32 = 16;
/// `--s-36`, the least width of a readout.
const READOUT_LEAST: u32 = 36;
/// A hairline, `--hair`.
const HAIR: u32 = 1;
/// The progress bar's least width, quire's `.ds-capsule-scrub` `min-width`.
const SCRUB_LEAST: u32 = 120;
/// The level's width, quire's `.ds-capsule-level` `width`.
const LEVEL: u32 = 96;
/// The widest a capsule that holds a progress bar grows, quire's `.ds-capsule[data-span=wide]`.
const WIDE_MOST: u32 = 640;

/// How soon a slot goes when the capsule is too wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rank {
    /// Never: the capsule is not worth showing without it.
    Stays,
    /// Dropped when the capsule does not fit; the higher the number, the sooner.
    Drops(u8),
}

/// A slot and how soon it goes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ranked<T> {
    pub(crate) rank: Rank,
    pub(crate) slot: CapsuleSlot<T>,
}

impl<T> Ranked<T> {
    /// A slot that is never dropped.
    pub(crate) fn stays(slot: CapsuleSlot<T>) -> Self {
        Self {
            rank: Rank::Stays,
            slot,
        }
    }

    /// A slot dropped at `rank`, the sooner the higher.
    pub(crate) fn drops(rank: u8, slot: CapsuleSlot<T>) -> Self {
        Self {
            rank: Rank::Drops(rank),
            slot,
        }
    }
}

/// The whole pixels of width a stage has, once it is measured.
pub(crate) fn stage_width(area: Option<Area>) -> Option<u32> {
    area.map(|area| area.size.width.0.floor().max(0.0) as u32)
        .filter(|width| *width > 0)
}

/// How wide one slot draws.
fn width_of<T>(slot: &CapsuleSlot<T>) -> u32 {
    match slot {
        CapsuleSlot::Item(_) => u32::from(ControlSize::Large.scale().height.0),
        CapsuleSlot::Readout(text) => {
            // Tabular figures, a little over half the font size each.
            let glyph = u32::from(ControlSize::Large.scale().font.0) * 6 / 10;
            let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
            READOUT_LEAST.max(chars.saturating_mul(glyph).saturating_add(S4 * 2))
        }
        CapsuleSlot::Divider => HAIR + S4 * 2,
        CapsuleSlot::Scrub(_) => SCRUB_LEAST,
        CapsuleSlot::Level(_) => LEVEL,
    }
}

/// Which of `ranked` show: all but a divider at an end or beside another.
fn showing<T>(ranked: &[Ranked<T>]) -> Vec<bool> {
    let mut showing = Vec::with_capacity(ranked.len());
    // The last slot that shows, and where it is.
    let mut last: Option<(usize, bool)> = None;
    for (at, one) in ranked.iter().enumerate() {
        let divider = matches!(one.slot, CapsuleSlot::Divider);
        let shows = !(divider && last.is_none_or(|(_, was_divider)| was_divider));
        if shows {
            last = Some((at, divider));
        }
        showing.push(shows);
    }
    if let Some((at, true)) = last
        && let Some(flag) = showing.get_mut(at)
    {
        *flag = false;
    }
    showing
}

/// The slots of `ranked` that show, by reference.
fn shown<T>(ranked: &[Ranked<T>]) -> Vec<&CapsuleSlot<T>> {
    ranked
        .iter()
        .zip(showing(ranked))
        .filter_map(|(one, shows)| shows.then_some(&one.slot))
        .collect()
}

/// Whether `slots` fit a stage `stage` pixels wide at their narrowest: the capsule's padding, a
/// gap between slots and each slot at its least, in the room the stage leaves, which a capsule
/// with a progress bar caps.
fn fit_stage<T>(slots: &[&CapsuleSlot<T>], stage: u32) -> bool {
    let gaps = u32::try_from(slots.len().saturating_sub(1)).unwrap_or(u32::MAX) * S4;
    let least = slots
        .iter()
        .map(|slot| width_of(slot))
        .fold(S8 * 2 + gaps, u32::saturating_add);
    let room = stage.saturating_sub(S16 * 2);
    let has_bar = slots
        .iter()
        .any(|slot| matches!(slot, CapsuleSlot::Scrub(_)));
    least <= if has_bar { room.min(WIDE_MOST) } else { room }
}

/// The most droppable rank still in `ranked`, if any is.
fn worst<T>(ranked: &[Ranked<T>]) -> Option<u8> {
    ranked
        .iter()
        .filter_map(|one| match one.rank {
            Rank::Drops(rank) => Some(rank),
            Rank::Stays => None,
        })
        .max()
}

/// The slots of `ranked` that fit a stage `stage` pixels wide, in order. The most droppable rank
/// goes first, all of it at once, and dividers left with nothing to divide go with it. With no
/// width to go by (the stage is not measured yet) every slot stays.
pub(crate) fn fit_slots<T>(mut ranked: Vec<Ranked<T>>, stage: Option<u32>) -> Vec<CapsuleSlot<T>> {
    if let Some(stage) = stage {
        while let Some(worst) = worst(&ranked) {
            if fit_stage(&shown(&ranked), stage) {
                break;
            }
            ranked.retain(|one| one.rank != Rank::Drops(worst));
        }
    }
    let showing = showing(&ranked);
    ranked
        .into_iter()
        .zip(showing)
        .filter_map(|(one, shows)| shows.then_some(one.slot))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds::components::chrome::capsule::model::{LevelSlot, ScrubSlot};
    use ds::motion::spring::Millis;
    use ds::prelude::Icon;
    use ds_core::vocab::{Availability, Fraction};

    fn button(rank: Option<u8>, name: &str) -> Ranked<String> {
        let slot = CapsuleSlot::button(name.to_owned(), name, Icon::Play);
        match rank {
            Some(rank) => Ranked::drops(rank, slot),
            None => Ranked::stays(slot),
        }
    }

    fn divider() -> Ranked<String> {
        Ranked::stays(CapsuleSlot::Divider)
    }

    fn readout(rank: u8, text: &str) -> Ranked<String> {
        Ranked::drops(rank, CapsuleSlot::Readout(text.to_owned()))
    }

    /// A player's capsule as the media family ranks it: the stays are play and the bar.
    fn player() -> Vec<Ranked<String>> {
        vec![
            button(None, "back"),
            button(None, "play"),
            button(None, "forward"),
            divider(),
            readout(1, "0:25"),
            Ranked::stays(CapsuleSlot::Scrub(ScrubSlot {
                label: "Position".to_owned(),
                position: Fraction(0),
                length: Millis(1000),
                buffered: Vec::new(),
                availability: Availability::Enabled,
            })),
            readout(3, "1:40"),
            divider(),
            Ranked::drops(
                2,
                CapsuleSlot::Level(LevelSlot {
                    label: "Volume".to_owned(),
                    value: Fraction(500),
                    availability: Availability::Enabled,
                }),
            ),
            divider(),
            button(Some(4), "slower"),
            readout(4, "1×"),
            button(Some(4), "faster"),
            divider(),
            button(Some(5), "export"),
        ]
    }

    fn names(slots: &[CapsuleSlot<String>]) -> Vec<String> {
        slots
            .iter()
            .map(|slot| match slot {
                CapsuleSlot::Item(item) => item.label.clone(),
                CapsuleSlot::Readout(text) => format!("[{text}]"),
                CapsuleSlot::Divider => "|".to_owned(),
                CapsuleSlot::Scrub(_) => "bar".to_owned(),
                CapsuleSlot::Level(_) => "level".to_owned(),
            })
            .collect()
    }

    #[test]
    fn a_narrower_stage_drops_the_least_important_first() {
        const CASES: &[(&str, u32, &[&str])] = &[
            (
                "a wide window: all of it",
                900,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level", "|",
                    "slower", "[1×]", "faster", "|", "export",
                ],
            ),
            (
                "the widest the capsule goes: all of it",
                720,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level", "|",
                    "slower", "[1×]", "faster", "|", "export",
                ],
            ),
            (
                "the export goes first, the stage a pixel short of the whole capsule's 640 and its 32 of margin",
                671,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level", "|",
                    "slower", "[1×]", "faster",
                ],
            ),
            (
                "the speed stays a while after the export",
                640,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level", "|",
                    "slower", "[1×]", "faster",
                ],
            ),
            (
                "the speed is gone at 560",
                560,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level",
                ],
            ),
            (
                "the least a window is: the length is gone too",
                480,
                &[
                    "back", "play", "forward", "|", "[0:25]", "bar", "|", "level",
                ],
            ),
        ];
        for (name, stage, want) in CASES {
            let slots = fit_slots(player(), Some(*stage));
            assert_eq!(names(&slots), *want, "{name} at {stage}");
        }
    }

    #[test]
    fn a_player_without_a_speed_or_an_export_fits_whole_when_the_stage_is_wide() {
        let mut slots = player();
        slots.retain(|one| !matches!(one.rank, Rank::Drops(4 | 5)));
        let slots = fit_slots(slots, Some(900));
        assert_eq!(
            names(&slots),
            [
                "back", "play", "forward", "|", "[0:25]", "bar", "[1:40]", "|", "level"
            ]
        );
    }

    #[test]
    fn each_rank_goes_in_its_turn_as_the_stage_narrows() {
        // The first stage width (counting down) at which each control is gone.
        let gone = |name: &str| {
            (0..900_u32).rev().find(|stage| {
                !names(&fit_slots(player(), Some(*stage)))
                    .iter()
                    .any(|shown| shown == name)
            })
        };
        let order: Vec<Option<u32>> = ["export", "slower", "[1:40]", "level", "[0:25]"]
            .iter()
            .map(|name| gone(name))
            .collect();
        assert!(
            order.windows(2).all(|pair| pair[0] >= pair[1]),
            "export, speed, length, level, clock: {order:?}"
        );
    }

    #[test]
    fn the_play_button_and_the_bar_stay_however_narrow() {
        for stage in [0, 100, 200, 300] {
            let slots = fit_slots(player(), Some(stage));
            assert_eq!(
                names(&slots),
                ["back", "play", "forward", "|", "bar"],
                "at {stage}"
            );
        }
    }

    #[test]
    fn a_stage_not_measured_yet_shows_everything() {
        assert_eq!(fit_slots(player(), None).len(), player().len());
    }

    #[test]
    fn no_divider_is_left_at_an_end_or_beside_another() {
        for stage in (0..1000).step_by(10) {
            let slots = fit_slots(player(), Some(stage));
            let divided = |at: usize| matches!(slots.get(at), Some(CapsuleSlot::Divider));
            assert!(!divided(0), "first at {stage}");
            assert!(
                !slots.is_empty() && !divided(slots.len() - 1),
                "last at {stage}"
            );
            assert!(
                (1..slots.len()).all(|at| !(divided(at) && divided(at - 1))),
                "doubled at {stage}"
            );
        }
    }

    #[test]
    fn what_is_kept_fits_the_room() {
        for stage in (480..1000).step_by(10) {
            let slots = fit_slots(player(), Some(stage));
            let refs: Vec<&CapsuleSlot<String>> = slots.iter().collect();
            assert!(fit_stage(&refs, stage), "at {stage}");
        }
    }
}
