//! What the window itself holds, as opposed to what a machine does: where a probe stands, the
//! document the load opened, the window of lines last read. These are the results of effects,
//! kept so the views can draw them and so a late result for a file the person left is dropped.

use crate::context::entries;
use crate::families::{LineWindow, LoadedDoc, family_of, views_of};
use crate::io::Probed;
use crate::{
    ChromeParams, Command, ContextParams, EditOffer, FileAccess, MediaOffer, Motion, PaletteParams,
    PanelParams, PlatformAbilities, PresentationParams, SheetParams, Spot, Stage, StageCommand,
    StageParams, TextParams, TextViews, Ticket, TypedText, ViewerParams,
};
use anyview_core::{FileAction, FormatKind, Reach, actions_for, reach};
use ds::prelude::MotionLevel;
use ds_core::word::Word;

/// Where the probe of the load in flight stands. A probe's result is announced to the load machine
/// only after the window has drawn once with it, so the parameters the machine steps with already
/// know the file's kind (which views a text file has).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Probe {
    /// No file has been asked for.
    Idle,
    /// The probe of this ticket is running.
    Pending(Ticket),
    /// The probe of this ticket is running again for a file that changed; the second is what the
    /// file on screen was probed as.
    Reprobing(Ticket, Probed),
    /// The probe answered; the load machine has not been told yet.
    Arrived(Ticket, Probed),
    /// The load machine knows.
    Announced(Ticket, Probed),
}

impl Probe {
    /// What the probe found, once it has.
    pub(super) fn found(&self) -> Option<&Probed> {
        match self {
            Probe::Arrived(_, probed)
            | Probe::Announced(_, probed)
            | Probe::Reprobing(_, probed) => Some(probed),
            Probe::Idle | Probe::Pending(_) => None,
        }
    }

    /// The load this probe belongs to.
    pub(super) fn ticket(&self) -> Option<Ticket> {
        match self {
            Probe::Pending(ticket)
            | Probe::Reprobing(ticket, _)
            | Probe::Arrived(ticket, _)
            | Probe::Announced(ticket, _) => Some(*ticket),
            Probe::Idle => None,
        }
    }
}

/// Whether the open recording can be played: without the player (or the plugin that reads it) it
/// shows its facts, and nothing that plays is on offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Playback {
    /// There is a player, or the file is not a recording.
    Playable,
    /// A recording nothing here plays.
    Unplayable,
}

/// What the viewer offers of the file actions: Copy File waits until the desktop's clipboard can
/// hold a file (the host's clipboard carries text only), what plays waits for a player, and what
/// needs a desktop service waits for the platform to have it.
fn offered(action: FileAction, playback: Playback, platform: PlatformAbilities) -> bool {
    if !platform.offers(action) {
        return false;
    }
    match action {
        FileAction::CopyFile => false,
        FileAction::PlayInMiniWindow | FileAction::PlayInBackground => {
            playback == Playback::Playable
        }
        FileAction::Open
        | FileAction::OpenWith
        | FileAction::RevealInFolder
        | FileAction::CopyPath
        | FileAction::Share
        | FileAction::Rename
        | FileAction::Duplicate
        | FileAction::MoveToTrash
        | FileAction::Print
        | FileAction::Export
        | FileAction::SaveCopy
        | FileAction::RevertTo
        | FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical
        | FileAction::ConvertTo => true,
    }
}

/// What the window's settings and its player say right now.
#[derive(Debug, Clone, Copy)]
pub(super) struct Live {
    /// How much the window moves.
    pub level: MotionLevel,
    /// What the player can do.
    pub abilities: crate::MediaAbilities,
    /// What the platform can do.
    pub platform: PlatformAbilities,
}

/// What the open file allows and offers, as the commands' filters read it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Offers {
    /// Whether the file can be played.
    pub playback: Playback,
    /// What a picture edit costs, or that none is possible.
    pub edit: EditOffer,
    /// Whether the file takes a save in place.
    pub access: FileAccess,
    /// The desktop services there are for the file actions that need one.
    pub platform: PlatformAbilities,
}

/// The commands the palette lists for a file of `kind` showing `stage`: the file actions the
/// viewer offers for the kind, then the stage's commands it has, in the shared order. What
/// changes the file in place is left out of a file that refuses a save.
pub(super) fn commands(
    kind: Option<FormatKind>,
    stage: &Stage,
    params: &StageParams,
    offers: Offers,
) -> Vec<Command> {
    let Offers {
        playback,
        edit: offer,
        access,
        platform,
    } = offers;
    let files = kind
        .map(actions_for)
        .unwrap_or_default()
        .iter()
        .filter(|action| offer != EditOffer::Withheld || !is_picture_edit(**action))
        .filter(|action| access == FileAccess::Writable || !saves_in_place(**action))
        .filter(|action| match reach(**action) {
            Reach::Viewer | Reach::Both => offered(**action, playback, platform),
            Reach::Launcher => false,
        })
        .map(|action| Command::File(*action));
    let stages = StageCommand::ALL
        .iter()
        .filter(|_| playback == Playback::Playable)
        .filter(|command| access == FileAccess::Writable || !edits_pages(**command))
        .filter(|command| stage.input_for(**command, params).is_some())
        .map(|command| Command::Stage(*command));
    let open = platform.pick_files.then_some(Command::OpenFile);
    open.into_iter().chain(files).chain(stages).collect()
}

/// Whether `action` turns or flips the picture.
fn is_picture_edit(action: FileAction) -> bool {
    matches!(
        action,
        FileAction::RotateLeft
            | FileAction::RotateRight
            | FileAction::FlipHorizontal
            | FileAction::FlipVertical
    )
}

/// Whether `action` writes the open file itself.
fn saves_in_place(action: FileAction) -> bool {
    is_picture_edit(action) || action == FileAction::RevertTo
}

/// Whether `command` changes the pages of the PDF it is run on.
fn edits_pages(command: StageCommand) -> bool {
    matches!(
        command,
        StageCommand::DeletePage | StageCommand::MovePageEarlier | StageCommand::MovePageLater
    )
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

/// Everything the machines read besides their inputs, from what the window knows now.
pub(super) fn params(
    stage: &Stage,
    doc: Option<&LoadedDoc>,
    probe: &Probe,
    area: Option<crate::Area>,
    query: &TypedText,
    lines: Option<&LineWindow>,
    live: Live,
) -> ViewerParams {
    let Live {
        level,
        abilities,
        platform,
    } = live;
    let kind = probe.found().map(|probed| probed.sniffed.kind());
    let playback = doc.map_or(Playback::Playable, |doc| match (kind, doc.view().line()) {
        (Some(FormatKind::Video | FormatKind::Audio), None) => Playback::Unplayable,
        _ => Playback::Playable,
    });
    let offer = doc.map_or(EditOffer::Plain, |doc| doc.view().edit_offer());
    let access = probe
        .found()
        .map_or(FileAccess::Writable, |probed| probed.access);
    let mut measured = match doc {
        Some(doc) => doc.view().params(stage, area, lines),
        None => StageParams {
            text: TextParams {
                views: kind.map_or(TextViews::default(), views_of),
                ..TextParams::default()
            },
            ..StageParams::default()
        },
    };
    measured.media.abilities = abilities;
    measured.raster.motion = match level {
        MotionLevel::Reduced => Motion::Reduced,
        MotionLevel::Standard => Motion::Standard,
    };
    let offers = Offers {
        playback,
        edit: offer,
        access,
        platform,
    };
    let listed = commands(kind, stage, &measured, offers);
    let mut panel = doc.map_or_else(PanelParams::default, |doc| doc.view().panel_params());
    if matches!(stage, Stage::Media(_)) {
        panel.tabs = crate::families::media_tabs(panel.tabs, abilities);
    }
    let files = listed
        .iter()
        .copied()
        .filter_map(|command| match command {
            Command::File(action) => Some(action),
            Command::Stage(_) | Command::OpenFile | Command::Install(_) => None,
        })
        .collect();
    ViewerParams {
        files,
        chrome: ChromeParams::default(),
        context: ContextParams {
            entries: entries(&listed, panel.tabs),
            centre: centre_of(area),
        },
        palette: PaletteParams {
            rows: ranked(listed, query),
        },
        panel,
        platform,
        presentation: PresentationParams::default(),
        sheet: SheetParams {
            media: doc.map_or_else(MediaOffer::default, |doc| doc.view().media_offer()),
            edit: offer,
        },
        stage: measured,
    }
}

/// The middle of the content, where a menu opened by a key goes: the window's own middle until
/// the content has been measured.
fn centre_of(area: Option<crate::Area>) -> Spot {
    area.map_or_else(Spot::default, |area| Spot {
        x: (area.origin.x.0 + area.size.width.0 / 2.0).round() as i32,
        y: (area.origin.y.0 + area.size.height.0 / 2.0).round() as i32,
    })
}

/// The family of stage a probed file gets.
pub(super) fn family(probed: &Probed) -> crate::StageFamily {
    family_of(probed.sniffed.kind())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_names_a_label_by_its_letters_in_order() {
        // name, label, query, whether it names it
        const CASES: &[(&str, &str, &str, bool)] = &[
            ("empty names everything", "Rotate Left", "", true),
            ("a prefix", "Rotate Left", "rot", true),
            ("letters in order", "Rotate Left", "rl", true),
            ("case is ignored", "Rotate Left", "ROT", true),
            ("spaces are ignored", "Rotate Left", "rot left", true),
            ("out of order", "Rotate Left", "lr", false),
            ("a letter the label lacks", "Rotate Left", "z", false),
        ];
        for (name, label, query, want) in CASES {
            assert_eq!(names(label, query), *want, "{name}");
        }
    }
}
