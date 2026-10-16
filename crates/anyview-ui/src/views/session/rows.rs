//! The capsule's slots and the palette's rows: which buttons the file takes, and which commands a
//! typed query names.

use crate::{Command, HitIndex, HitList, PaletteScope, Stage, StageCommand, TypedText};
use anyview_core::{FileAction, FindOption, FindOptions};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;

/// The capsule's `slots` without the buttons for a file action the file does not take (`files` is
/// what the palette and the menu list), and without the dividers that leave: none first or last,
/// and never two together. Each button that has a key carries it, so its tooltip shows it (Zoom
/// to Fit, ⌘9) whatever family built the slot.
pub(in crate::views) fn offered_slots(
    slots: Vec<RankedSlot<Command>>,
    files: &[FileAction],
) -> Vec<RankedSlot<Command>> {
    let mut kept: Vec<RankedSlot<Command>> = Vec::with_capacity(slots.len());
    for mut ranked in slots {
        if let CapsuleSlot::Item(item) = &mut ranked.slot
            && item.shortcut.is_none()
        {
            item.shortcut = item.value.shortcut();
        }
        let taken = match &ranked.slot {
            CapsuleSlot::Item(item) => match item.value {
                Command::File(action) => files.contains(&action),
                Command::Stage(_)
                | Command::OpenFile
                | Command::UseTool(_)
                | Command::Picture(_)
                | Command::Markup(_)
                | Command::ShowView(_)
                | Command::FindHit(_)
                | Command::ShowAllHits
                | Command::FindOption(_)
                | Command::Install(_) => true,
            },
            CapsuleSlot::Readout(_)
            | CapsuleSlot::Divider
            | CapsuleSlot::Scrub(_)
            | CapsuleSlot::Level(_) => true,
        };
        if !taken {
            continue;
        }
        let doubled = matches!(ranked.slot, CapsuleSlot::Divider)
            && kept
                .last()
                .is_none_or(|last| matches!(last.slot, CapsuleSlot::Divider));
        if !doubled {
            kept.push(ranked);
        }
    }
    if kept
        .last()
        .is_some_and(|last| matches!(last.slot, CapsuleSlot::Divider))
    {
        kept.pop();
    }
    kept
}

/// The capsule's `slots` without the Find button, for a pane, which has no palette to find in.
pub(in crate::views) fn without_find(slots: Vec<RankedSlot<Command>>) -> Vec<RankedSlot<Command>> {
    slots
        .into_iter()
        .filter(|ranked| match &ranked.slot {
            CapsuleSlot::Item(item) => item.value != Command::Stage(StageCommand::Find),
            CapsuleSlot::Readout(_)
            | CapsuleSlot::Divider
            | CapsuleSlot::Scrub(_)
            | CapsuleSlot::Level(_) => true,
        })
        .collect()
}

/// Whether `query` names `label`: every letter of the query, in order, ignoring case.
pub(super) fn names(label: &str, query: &str) -> bool {
    let mut wanted = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase);
    let mut next = wanted.next();
    for c in label.chars().flat_map(char::to_lowercase) {
        if next == Some(c) {
            next = wanted.next();
        }
    }
    next.is_none()
}

/// The commands that `query` names, in their order.
pub(super) fn ranked(commands: Vec<Command>, query: &TypedText) -> Vec<Command> {
    commands
        .into_iter()
        .filter(|command| names(&command.label(), query.as_str()))
        .collect()
}

/// How many hits "In This File" lists before "Show All".
pub(in crate::views) const BRIEF_HITS: u32 = 8;

/// The most hits "Show All" lists: a palette is a list to glance down, not the whole document.
pub(in crate::views) const WHOLE_HITS: u32 = 200;

/// The palette's rows for `query`: the commands it names, and for a find the hits of the file first
/// (a few, then a row to list them all), the way mailo's search lists mail before commands.
pub(super) fn rows_for(
    listed: Vec<Command>,
    query: &TypedText,
    scope: PaletteScope,
    finding: (&Stage, FindOptions),
) -> Vec<Command> {
    let (stage, options) = finding;
    match scope {
        PaletteScope::Commands => ranked(listed, query),
        PaletteScope::Find(list) => {
            let found = stage
                .find_state()
                .and_then(|(_, hits)| hits.count())
                .map_or(0, |count| count.0);
            let shown = match list {
                HitList::Brief => found.min(BRIEF_HITS),
                HitList::Whole => found.min(WHOLE_HITS),
            };
            let more =
                (list == HitList::Brief && found > BRIEF_HITS).then_some(Command::ShowAllHits);
            // Something to narrow, or an option to undo: switching one on can only lose hits, so a
            // find with none and every option off has no use for them.
            let refine = (found > 0 || options != FindOptions::default())
                .then(|| FindOption::ALL.map(Command::FindOption))
                .into_iter()
                .flatten();
            let commands: Vec<Command> = listed
                .into_iter()
                .filter(|command| *command != Command::Stage(StageCommand::Find))
                .collect();
            (0..shown)
                .map(|hit| Command::FindHit(HitIndex(hit)))
                .chain(more)
                .chain(refine)
                .chain(ranked(commands, query))
                .collect()
        }
    }
}
