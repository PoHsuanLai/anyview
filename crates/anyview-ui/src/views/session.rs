//! What the window itself holds, as opposed to what a machine does: where a probe stands, the
//! document the load opened, the window of lines last read. These are the results of effects,
//! kept so the views can draw them and so a late result for a file the person left is dropped.

use crate::families::{LineWindow, LoadedDoc, family_of, views_of};
use crate::io::Probed;
use crate::{
    ChromeParams, Command, PaletteParams, PanelParams, PresentationParams, Stage, StageCommand,
    StageParams, TextParams, TextViews, Ticket, TypedText, ViewerParams,
};
use anyview_core::{FormatKind, Reach, actions_for, reach};
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

/// The commands the palette lists for a file of `kind` showing `stage`: the file actions the
/// viewer offers for the kind, then the stage's commands it has, in the shared order.
pub(super) fn commands(
    kind: Option<FormatKind>,
    stage: &Stage,
    params: &StageParams,
) -> Vec<Command> {
    let files = kind
        .map(actions_for)
        .unwrap_or_default()
        .iter()
        .filter(|action| match reach(**action) {
            Reach::Viewer | Reach::Both => true,
            Reach::Launcher => false,
        })
        .map(|action| Command::File(*action));
    let stages = StageCommand::ALL
        .iter()
        .filter(|command| stage.input_for(**command, params).is_some())
        .map(|command| Command::Stage(*command));
    files.chain(stages).collect()
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
) -> ViewerParams {
    let kind = probe.found().map(|probed| probed.sniffed.kind());
    let measured = match doc {
        Some(doc) => doc.view().params(stage, area, lines),
        None => StageParams {
            text: TextParams {
                views: kind.map_or(TextViews::default(), views_of),
                ..TextParams::default()
            },
            ..StageParams::default()
        },
    };
    ViewerParams {
        chrome: ChromeParams::default(),
        panel: doc.map_or_else(PanelParams::default, |doc| doc.view().panel_params()),
        palette: PaletteParams {
            rows: ranked(commands(kind, stage, &measured), query),
        },
        presentation: PresentationParams::default(),
        stage: measured,
    }
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
