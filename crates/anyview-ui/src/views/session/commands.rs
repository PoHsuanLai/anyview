//! The commands the palette and the context menu list for the open file and its stage.

use super::offers::{MarkupOffer, Offers, PictureOffer, Playback, offered};
use crate::io::DesktopService;
use crate::{
    Allowance, Changes, Command, FileAccess, MarkupCommand, Stage, StageCommand, StageParams,
    TextView, TextViews, Writing,
};
use anyview_core::{FormatKind, MarkTool, Reach, actions_for, reach};
use ds_core::word::Word;

/// The commands the palette lists for a file of `kind` showing `stage`: the file actions the
/// viewer offers for the kind, then the stage's commands it has, in the shared order. What
/// changes the file in place is left out of a file that refuses a save.
pub(super) fn commands(
    kind: Option<FormatKind>,
    stage: &Stage,
    params: &StageParams,
    offers: Offers,
) -> Vec<Command> {
    let ability = stage.abilities();
    let files = kind
        .map(actions_for)
        .unwrap_or_default()
        .iter()
        .filter(|action| match reach(**action) {
            Reach::Viewer | Reach::Both => offered(**action, ability, offers),
            Reach::Launcher => false,
        })
        .map(|action| Command::File(*action));
    let stages = StageCommand::ALL
        .iter()
        .filter(|_| offers.playback == Playback::Playable)
        .filter(|command| offers.access == FileAccess::Writable || !changes_file(**command))
        // The two views of a text file are listed as "Show Preview" and "Show Source" below.
        .filter(|command| **command != StageCommand::ToggleSource)
        // The arrows between cells are keys, not rows of the palette.
        .filter(|command| !matches!(command, StageCommand::PreviousCell | StageCommand::NextCell))
        .filter(|command| stage.input_for(**command, params).is_some())
        .map(|command| Command::Stage(*command));
    let views = match (stage, params.text.views) {
        // The text being edited is its source: there is no page to switch to.
        (Stage::Text(text), TextViews::RenderedAndSource) if text.edited().is_none() => {
            Some(Command::ShowView(match text.place().view {
                TextView::Rendered => TextView::Source,
                TextView::Source => TextView::Rendered,
            }))
        }
        (Stage::Text(_), TextViews::RenderedAndSource | TextViews::SourceOnly)
        | (
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Table(_)
            | Stage::Tree(_),
            _,
        ) => None,
    };
    let open = offers
        .platform
        .has(DesktopService::FileChooser)
        .then_some(Command::OpenFile);
    let pan = (offers.playback == Playback::Playable && matches!(stage, Stage::Raster(_)))
        .then_some(Command::UseTool(offers.tool.other()));
    open.into_iter()
        .chain(files)
        .chain(stages)
        .chain(views)
        .chain(pan)
        .collect()
}

/// The commands for editing the open picture: the crop tool, a new size, and saving what has been
/// done. They are listed when the picture can be saved with changes; Save only once there are some.
pub(super) fn picture_commands(
    stage: &Stage,
    offer: PictureOffer,
    tool: crate::Tool,
) -> Vec<Command> {
    if !matches!(stage, Stage::Raster(_)) {
        return Vec::new();
    }
    match offer {
        PictureOffer::Unavailable => Vec::new(),
        PictureOffer::Pristine | PictureOffer::Edited => {
            let crop = (tool != crate::Tool::Crop).then_some(Command::UseTool(crate::Tool::Crop));
            let save = (offer == PictureOffer::Edited)
                .then_some(Command::Picture(crate::PictureCommand::Save));
            crop.into_iter()
                .chain([Command::Picture(crate::PictureCommand::AdjustSize)])
                .chain(save)
                .collect()
        }
    }
}

/// The tools a mark can be made with, in the order the palette lists them.
pub(super) const MARK_TOOLS: [MarkTool; 10] = [
    MarkTool::Highlight,
    MarkTool::Underline,
    MarkTool::Strikethrough,
    MarkTool::Note,
    MarkTool::Rectangle,
    MarkTool::Oval,
    MarkTool::Line,
    MarkTool::Arrow,
    MarkTool::Sketch,
    MarkTool::TextBox,
];

/// The commands for marking the open PDF: a tool for each kind of mark where the document lets the
/// person mark it, Delete Mark while one is picked, and Save once there is something to save.
pub(super) fn markup_commands(stage: &Stage, offer: MarkupOffer) -> Vec<Command> {
    if stage.abilities().writing != Writing::Writable {
        return Vec::new();
    }
    let tools = (offer.marking == Allowance::Granted)
        .then_some(MARK_TOOLS)
        .into_iter()
        .flatten()
        .map(|tool| Command::Markup(MarkupCommand::Tool(tool)));
    let delete = offer.picked.map(|_| Command::Markup(MarkupCommand::Delete));
    let save = (offer.changes == Changes::Unsaved).then_some(Command::Markup(MarkupCommand::Save));
    tools.chain(delete).chain(save).collect()
}

/// Whether `command` changes the file it is run on: the pages of a PDF, the text of a text.
pub(super) fn changes_file(command: StageCommand) -> bool {
    matches!(
        command,
        StageCommand::DeletePage
            | StageCommand::MovePageEarlier
            | StageCommand::MovePageLater
            | StageCommand::Edit
            | StageCommand::Save
            | StageCommand::AddRow
            | StageCommand::DeletePicked
            | StageCommand::AddItem
            | StageCommand::RenameKey
    )
}
