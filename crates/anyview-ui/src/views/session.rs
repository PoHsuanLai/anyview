//! What the window itself holds, as opposed to what a machine does: where a probe stands, the
//! document the load opened, the window of lines last read. These are the results of effects,
//! kept so the views can draw them and so a late result for a file the person left is dropped.

use crate::families::{LineWindow, LoadedDoc, family_of, views_of};
use crate::io::{DesktopService, NaturalSize, Opened};
use crate::{
    ChromeParams, Command, ContextParams, EditOffer, ExportFacts, FileAccess, HitIndex, HitList,
    MediaOffer, Motion, Palette, PaletteParams, PaletteScope, PaneChrome, PanelParams,
    PlatformAbilities, Playing, Presentation, PresentationParams, SheetParams, Spot, Stage,
    StageAbilities, StageCommand, StageParams, TextParams, TextView, TextViews, Ticket, TypedText,
    ViewerParams,
};
use anyview_core::{Adjust, FileAction, FormatKind, Reach, actions_for, reach};
use anyview_machines::seam::{WrapChoices, entries};
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;
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
    Reprobing(Ticket, Opened),
    /// The probe answered; the load machine has not been told yet.
    Arrived(Ticket, Opened),
    /// The load machine knows.
    Announced(Ticket, Opened),
}

impl Probe {
    /// What the probe found, once it has.
    pub(super) fn found(&self) -> Option<&Opened> {
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

/// Whether the Export sheet has anything to write: a recording with no export on offer and no
/// package to name has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exports {
    /// The sheet has a format to write.
    Offered,
    /// The sheet would be empty.
    Nothing,
}

/// Whether the open file takes `action`, from what the showing stage declares it can do
/// (`Stage::abilities`), what the file allows and what the platform has. One match, so a file
/// action added to the vocabulary has to say where it applies, and nothing is listed that would do
/// nothing: Copy File waits until the desktop's clipboard can hold a file (the host's clipboard
/// carries text only), what plays waits for a player, what edits waits for a stage that edits and a
/// file that takes a save, and what needs a desktop service waits for the platform to have it.
fn offered(action: FileAction, ability: StageAbilities, offers: Offers) -> bool {
    let Offers {
        playback,
        edit,
        access,
        exports,
        platform,
        tool: _,
    } = offers;
    if !platform.offers(action) {
        return false;
    }
    let writable = access == FileAccess::Writable;
    match action {
        FileAction::CopyFile => false,
        FileAction::PlayInMiniWindow | FileAction::PlayInBackground => {
            ability.playing == Playing::Plays && playback == Playback::Playable
        }
        FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical => {
            ability.edits.contains(&action) && edit != EditOffer::Withheld && writable
        }
        FileAction::RevertTo => !ability.edits.is_empty() && writable,
        FileAction::Export | FileAction::ConvertTo => {
            ability.export.is_some() && exports == Exports::Offered
        }
        FileAction::Open
        | FileAction::RevealInFolder
        | FileAction::CopyPath
        | FileAction::Share
        | FileAction::Rename
        | FileAction::Duplicate
        | FileAction::MoveToTrash
        | FileAction::Print
        | FileAction::SaveCopy => true,
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
    /// The pointer tool in use.
    pub tool: crate::Tool,
    /// What the person marked to keep of the recording.
    pub marks: crate::TrimMarks,
    /// What the open picture can be changed with.
    pub picture: PictureOffer,
    /// What has been done to the open picture.
    pub adjust: Adjust,
    /// How the person last chose to wrap each kind of text.
    pub wraps: WrapChoices,
    /// How the viewer is on screen: a pane is read-only and has no palette, so it lists no Find.
    pub presentation: Presentation,
    /// How much chrome a pane draws; a window ignores it.
    pub chrome: PaneChrome,
}

/// What editing the open picture offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PictureOffer {
    /// The file is not a picture that can be saved with changes.
    Unavailable,
    /// It can be, and nothing has been done to it.
    Pristine,
    /// It can be, and there are changes that are not saved.
    Edited,
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
    /// Whether the Export sheet has anything to write.
    pub exports: Exports,
    /// The desktop services there are for the file actions that need one.
    pub platform: PlatformAbilities,
    /// The pointer tool in use: the palette lists the other.
    pub tool: crate::Tool,
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
fn picture_commands(stage: &Stage, offer: PictureOffer, tool: crate::Tool) -> Vec<Command> {
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

/// Whether `command` changes the file it is run on: the pages of a PDF, the text of a text.
fn changes_file(command: StageCommand) -> bool {
    matches!(
        command,
        StageCommand::DeletePage
            | StageCommand::MovePageEarlier
            | StageCommand::MovePageLater
            | StageCommand::Edit
            | StageCommand::Save
    )
}

/// The capsule's `slots` without the buttons for a file action the file does not take (`files` is
/// what the palette and the menu list), and without the dividers that leave: none first or last,
/// and never two together. Each button that has a key carries it, so its tooltip shows it (Zoom
/// to Fit, ⌘9) whatever family built the slot.
pub(super) fn offered_slots(
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
                | Command::ShowView(_)
                | Command::FindHit(_)
                | Command::ShowAllHits
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
pub(super) fn without_find(slots: Vec<RankedSlot<Command>>) -> Vec<RankedSlot<Command>> {
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
const BRIEF_HITS: u32 = 8;

/// The most hits "Show All" lists: a palette is a list to glance down, not the whole document.
pub(super) const WHOLE_HITS: u32 = 200;

/// The palette's rows for `query`: the commands it names, and for a find the hits of the file first
/// (a few, then a row to list them all), the way mailo's search lists mail before commands.
fn rows_for(
    listed: Vec<Command>,
    query: &TypedText,
    scope: PaletteScope,
    stage: &Stage,
) -> Vec<Command> {
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
            let commands: Vec<Command> = listed
                .into_iter()
                .filter(|command| *command != Command::Stage(StageCommand::Find))
                .collect();
            (0..shown)
                .map(|hit| Command::FindHit(HitIndex(hit)))
                .chain(more)
                .chain(ranked(commands, query))
                .collect()
        }
    }
}

/// Everything the machines read besides their inputs, from what the window knows now.
pub(super) fn params(
    stage: &Stage,
    doc: Option<&LoadedDoc>,
    probe: &Probe,
    area: Option<crate::Area>,
    palette: &Palette,
    lines: Option<&LineWindow>,
    live: Live,
) -> ViewerParams {
    let (query, scope) = match palette {
        Palette::Open {
            query,
            selection: _,
            scope,
        } => (query.clone(), *scope),
        Palette::Closed => (TypedText::EMPTY, PaletteScope::Commands),
    };
    let Live {
        level,
        abilities,
        platform,
        tool,
        marks,
        picture,
        adjust,
        wraps,
        presentation,
        chrome,
    } = live;
    let pane = presentation == Presentation::Pane;
    let kind = probe.found().map(|probed| probed.sniffed.kind());
    let playback = doc.map_or(Playback::Playable, |doc| match (kind, doc.view().line()) {
        (Some(FormatKind::Video | FormatKind::Audio), None) => Playback::Unplayable,
        _ => Playback::Playable,
    });
    let offer = doc.map_or(EditOffer::Plain, |doc| doc.view().edit_offer());
    // A pane shows a file and changes nothing in it: what edits is the host's to offer.
    let access = if pane {
        FileAccess::ReadOnly
    } else {
        probe
            .found()
            .map_or(FileAccess::Writable, |probed| probed.access)
    };
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
    if let (Stage::Raster(raster), Some(area), Some(doc), false) =
        (stage, area, doc, adjust.is_none())
        && let Some(NaturalSize::Pixels(base)) = doc.view().natural()
    {
        measured.raster =
            crate::families::adjusted_raster(measured.raster.clone(), raster, base, adjust, area);
    }
    if pane {
        measured.text.editable = crate::Editable::No;
    }
    measured.media.abilities = abilities;
    if let Some(kind) = kind {
        measured.text.wrap = wraps.of(kind);
    }
    measured.raster.motion = match level {
        MotionLevel::Reduced => Motion::Reduced,
        MotionLevel::Standard => Motion::Standard,
    };
    let exports = match (stage, doc) {
        (Stage::Media(_), Some(doc)) => {
            let media = doc.view().media_offer();
            if media.first().is_some() || media.needs().is_some() {
                Exports::Offered
            } else {
                Exports::Nothing
            }
        }
        _ => Exports::Offered,
    };
    let offers = Offers {
        playback,
        edit: offer,
        access,
        exports,
        platform,
        tool,
    };
    let mut listed = commands(kind, stage, &measured, offers);
    listed.extend(picture_commands(stage, picture, tool));
    if pane {
        // Finding is the palette's, and a pane draws none.
        listed.retain(|command| *command != Command::Stage(StageCommand::Find));
    }
    let mut panel = doc.map_or_else(PanelParams::default, |doc| doc.view().panel_params());
    if pane && chrome != PaneChrome::WithPanel {
        panel.tabs = crate::PanelTabs::default();
    }
    if matches!(stage, Stage::Media(_)) {
        panel.tabs = crate::families::media_tabs(panel.tabs, abilities);
    }
    let files = listed
        .iter()
        .copied()
        .filter_map(|command| match command {
            Command::File(action) => Some(action),
            Command::Stage(_)
            | Command::OpenFile
            | Command::UseTool(_)
            | Command::Picture(_)
            | Command::ShowView(_)
            | Command::FindHit(_)
            | Command::ShowAllHits
            | Command::Install(_) => None,
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
            rows: rows_for(listed, &query, scope, stage),
        },
        panel,
        platform,
        presentation: PresentationParams::default(),
        sheet: SheetParams {
            media: doc.map_or_else(MediaOffer::default, |doc| doc.view().media_offer()),
            edit: offer,
            export: export_facts(stage, doc, &measured, marks),
        },
        stage: measured,
    }
}

/// What the export sheet is told of the open file: the pages of a document and where the reader
/// is in it, the size of a picture, and the part of a recording marked to keep.
fn export_facts(
    stage: &Stage,
    doc: Option<&LoadedDoc>,
    measured: &StageParams,
    marks: crate::TrimMarks,
) -> ExportFacts {
    let mut facts = ExportFacts::default();
    match stage {
        Stage::Pdf(pdf) => {
            facts.pages = Some(measured.pdf.pages);
            facts.page = pdf.place().page;
        }
        Stage::Raster(_) => {
            facts.image = doc.and_then(|doc| match doc.view().natural() {
                Some(NaturalSize::Pixels(size)) => Some(size),
                Some(NaturalSize::Points(_) | NaturalSize::Compact(_)) | None => None,
            });
        }
        Stage::Media(_) => {
            facts.marks = marks.is_set().then(|| marks.range());
        }
        Stage::NoStage | Stage::Text(_) | Stage::Table(_) | Stage::Tree(_) => {}
    }
    facts
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
pub(super) fn family(probed: &Opened) -> crate::StageFamily {
    family_of(probed.sniffed.kind())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::family_of;
    use crate::{ContextEntry, ContextPick, PanelTab, PanelTabs, PlatformAbilities};
    use FileAction::{
        ConvertTo, CopyFile, CopyPath, Export, FlipHorizontal, FlipVertical, PlayInBackground,
        PlayInMiniWindow, Print, RevertTo, RotateLeft, RotateRight, SaveCopy, Share,
    };

    /// What a file of `kind` lists, by the palette (`commands`) and the context menu (`entries`).
    fn lists(kind: FormatKind, offers: Offers) -> (Vec<Command>, Vec<ContextEntry>) {
        let stage = Stage::for_family(family_of(kind), TextViews::default());
        let palette = commands(Some(kind), &stage, &StageParams::default(), offers);
        let menu = entries(&palette, PanelTabs::of(&[PanelTab::Info]));
        (palette, menu)
    }

    fn plain() -> Offers {
        Offers {
            playback: Playback::Playable,
            edit: EditOffer::Plain,
            access: FileAccess::Writable,
            exports: Exports::Offered,
            platform: PlatformAbilities::ALL,
            tool: crate::Tool::default(),
        }
    }

    /// The file actions the table speaks of, so a row names only those it has a view on.
    const ASKED: &[FileAction] = &[
        RotateLeft,
        RotateRight,
        FlipHorizontal,
        FlipVertical,
        Export,
        ConvertTo,
        Print,
        SaveCopy,
        RevertTo,
        PlayInMiniWindow,
        PlayInBackground,
        CopyFile,
        CopyPath,
        Share,
    ];

    fn asked(palette: &[Command]) -> Vec<FileAction> {
        ASKED
            .iter()
            .copied()
            .filter(|action| palette.contains(&Command::File(*action)))
            .collect()
    }

    #[test]
    fn each_family_lists_only_what_it_can_do_in_the_palette_and_the_menu() {
        use FormatKind::{
            Archive, Audio, Book, Code, Folder, Font, Markdown, Office, Other, Pdf, PlainText,
            Raster, Table, Tree, Vector, Video,
        };
        const COMMON: &[FileAction] = &[CopyPath, Share];
        let with = |extra: &[FileAction]| -> Vec<FileAction> {
            let mut all: Vec<FileAction> = ASKED
                .iter()
                .copied()
                .filter(|a| COMMON.contains(a) || extra.contains(a))
                .collect();
            all.dedup();
            all
        };
        // name, kind, the asked-about actions it lists
        let cases: Vec<(&str, FormatKind, Vec<FileAction>)> = vec![
            (
                "a picture turns, flips, prints, exports and converts",
                Raster,
                with(&[
                    RotateLeft,
                    RotateRight,
                    FlipHorizontal,
                    FlipVertical,
                    Export,
                    ConvertTo,
                    Print,
                    SaveCopy,
                    RevertTo,
                ]),
            ),
            (
                "a PDF turns its page, and does not flip",
                Pdf,
                with(&[
                    RotateLeft,
                    RotateRight,
                    Export,
                    ConvertTo,
                    Print,
                    SaveCopy,
                    RevertTo,
                ]),
            ),
            (
                "a vector picture has no edit and no conversion",
                Vector,
                with(&[Export, Print]),
            ),
            (
                "a video plays in a small window and exports",
                Video,
                with(&[Export, PlayInMiniWindow]),
            ),
            (
                "a song plays in the background, exports and converts",
                Audio,
                with(&[Export, ConvertTo, PlayInBackground]),
            ),
            (
                "Markdown, code and text export, convert and print",
                Markdown,
                with(&[Export, ConvertTo, Print]),
            ),
            ("code", Code, with(&[Export, ConvertTo, Print])),
            ("plain text", PlainText, with(&[Export, ConvertTo, Print])),
            ("a table has no export", Table, with(&[])),
            ("a tree has no export", Tree, with(&[])),
            ("a book has no export", Book, with(&[])),
            ("a font is a card of facts", Font, with(&[])),
            ("an archive is a card of facts", Archive, with(&[])),
            ("a document is a card of facts", Office, with(&[])),
            ("a folder is a card of facts", Folder, with(&[])),
            ("a file of no kind is a card of facts", Other, with(&[])),
        ];
        for (name, kind, want) in cases {
            let (palette, menu) = lists(kind, plain());
            assert_eq!(asked(&palette), want, "{name}: the palette");
            // The menu rows are the palette's commands, none other.
            for entry in &menu {
                if let ContextEntry::Item {
                    pick: ContextPick::Run(command),
                    title,
                } = entry
                {
                    assert!(
                        palette.contains(command),
                        "{name}: {title} is in the palette"
                    );
                }
            }
            let titles: Vec<&str> = menu
                .iter()
                .filter_map(|entry| match entry {
                    ContextEntry::Item { title, .. } => Some(*title),
                    ContextEntry::Separator => None,
                })
                .collect();
            let row = |title: &str| titles.contains(&title);
            assert_eq!(
                row("Rotate Left"),
                want.contains(&RotateLeft),
                "{name}: menu"
            );
            assert_eq!(
                row("Export\u{2026}"),
                want.contains(&Export),
                "{name}: menu"
            );
            assert!(row("Get Info"), "{name}: the info tab gives the row");
            assert!(
                !titles.iter().any(|t| t.contains("Open With")),
                "{name}: {titles:?}"
            );
            assert!(
                !titles.contains(&"Copy"),
                "{name}: no file is put on the clipboard yet"
            );
        }
    }

    #[test]
    fn a_file_that_cannot_be_saved_or_turned_or_played_lists_none_of_what_needs_it() {
        // name, kind, the offers, the asked-about actions it must not list
        let read_only = Offers {
            access: FileAccess::ReadOnly,
            ..plain()
        };
        let withheld = Offers {
            edit: EditOffer::Withheld,
            ..plain()
        };
        let unplayable = Offers {
            playback: Playback::Unplayable,
            ..plain()
        };
        let unexportable = Offers {
            exports: Exports::Nothing,
            ..plain()
        };
        let no_desktop = Offers {
            platform: PlatformAbilities::NONE,
            ..plain()
        };
        let cases: Vec<(&str, FormatKind, Offers, Vec<FileAction>)> = vec![
            (
                "a picture that refuses a save",
                FormatKind::Raster,
                read_only,
                vec![
                    RotateLeft,
                    RotateRight,
                    FlipHorizontal,
                    FlipVertical,
                    RevertTo,
                ],
            ),
            (
                "a PDF that refuses a save",
                FormatKind::Pdf,
                read_only,
                vec![RotateLeft, RotateRight, RevertTo],
            ),
            (
                "a picture whose edit cannot be made",
                FormatKind::Raster,
                withheld,
                vec![RotateLeft, RotateRight, FlipHorizontal, FlipVertical],
            ),
            (
                "a recording nothing plays",
                FormatKind::Video,
                unplayable,
                vec![PlayInMiniWindow],
            ),
            (
                "a recording with no export to offer",
                FormatKind::Video,
                unexportable,
                vec![Export, ConvertTo],
            ),
            (
                "a platform with no desktop services",
                FormatKind::Raster,
                no_desktop,
                vec![Print, Share],
            ),
        ];
        for (name, kind, offers, gone) in cases {
            let (palette, menu) = lists(kind, offers);
            for action in gone {
                assert!(
                    !palette.contains(&Command::File(action)),
                    "{name}: {action:?}"
                );
            }
            let in_menu = |title: &str| {
                menu.iter().any(
                    |entry| matches!(entry, ContextEntry::Item { title: t, .. } if *t == title),
                )
            };
            if offers.access == FileAccess::ReadOnly || offers.edit == EditOffer::Withheld {
                assert!(!in_menu("Rotate Left"), "{name}: menu");
            }
        }
    }

    #[test]
    fn a_text_lists_edit_when_it_can_be_edited_and_done_and_save_while_it_is() {
        use crate::{Editable, Edited, TextPlace, TextStage, Wrap};
        use anyview_core::LineIndex;
        let reading = Stage::Text(TextStage::default());
        let editing = Stage::Text(TextStage::Editing {
            place: TextPlace {
                line: LineIndex(0),
                wrap: Wrap::On,
                view: TextView::Source,
            },
            edited: Edited::default(),
            find: None,
        });
        let params = |editable| StageParams {
            text: TextParams {
                editable,
                ..TextParams::default()
            },
            ..StageParams::default()
        };
        let locked = Offers {
            access: FileAccess::ReadOnly,
            ..plain()
        };
        const THREE: [StageCommand; 3] =
            [StageCommand::Edit, StageCommand::Done, StageCommand::Save];
        // name, stage, whether the file can be edited, what the file allows, the three it lists
        let cases = [
            (
                "a text that can be edited lists Edit",
                &reading,
                Editable::Yes,
                plain(),
                vec![StageCommand::Edit],
            ),
            (
                "a text too large to edit lists none",
                &reading,
                Editable::No,
                plain(),
                vec![],
            ),
            (
                "a text that refuses a save lists none",
                &reading,
                Editable::Yes,
                locked,
                vec![],
            ),
            (
                "a text being edited lists Done and Save",
                &editing,
                Editable::Yes,
                plain(),
                vec![StageCommand::Done, StageCommand::Save],
            ),
        ];
        for (name, stage, editable, offers, want) in cases {
            let listed = commands(
                Some(FormatKind::PlainText),
                stage,
                &params(editable),
                offers,
            );
            let got: Vec<StageCommand> = THREE
                .into_iter()
                .filter(|command| listed.contains(&Command::Stage(*command)))
                .collect();
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn play_pause_is_listed_for_a_recording_only() {
        let stage = Stage::for_family(family_of(FormatKind::Raster), TextViews::default());
        let toggle = Command::Stage(StageCommand::TogglePlayback);
        let still = commands(
            Some(FormatKind::Raster),
            &stage,
            &StageParams::default(),
            plain(),
        );
        assert!(!still.contains(&toggle), "a still picture does not play");
        let media = Stage::for_family(family_of(FormatKind::Video), TextViews::default());
        let video = commands(
            Some(FormatKind::Video),
            &media,
            &StageParams::default(),
            plain(),
        );
        assert!(video.contains(&toggle), "a recording does");
        let pdf = Stage::for_family(family_of(FormatKind::Pdf), TextViews::default());
        let pages = commands(
            Some(FormatKind::Pdf),
            &pdf,
            &StageParams::default(),
            plain(),
        );
        assert!(!pages.contains(&toggle), "a PDF does not");
    }

    #[test]
    fn the_capsule_drops_the_file_buttons_the_palette_does_not_list_and_the_dividers_they_leave() {
        use ds::components::chrome::capsule::priority::SlotPriority;
        use ds::prelude::Icon;
        let button = |command: Command| {
            RankedSlot::new(
                SlotPriority::Essential,
                CapsuleSlot::button(command, "x", Icon::Plus),
            )
        };
        let divider = || RankedSlot::new(SlotPriority::Essential, CapsuleSlot::Divider);
        let zoom = Command::Stage(StageCommand::ZoomIn);
        let rotate = Command::File(RotateLeft);
        let slots = vec![
            button(zoom),
            divider(),
            button(rotate),
            button(Command::File(RotateRight)),
            divider(),
            button(Command::Stage(StageCommand::TogglePlayback)),
        ];
        let shape = |kept: &[RankedSlot<Command>]| -> Vec<String> {
            kept.iter()
                .map(|ranked| match &ranked.slot {
                    CapsuleSlot::Item(item) => format!("{:?}", item.value),
                    CapsuleSlot::Divider => "|".to_owned(),
                    CapsuleSlot::Readout(_) | CapsuleSlot::Scrub(_) | CapsuleSlot::Level(_) => {
                        "?".to_owned()
                    }
                })
                .collect()
        };
        let all = offered_slots(slots.clone(), &[RotateLeft, RotateRight]);
        assert_eq!(all.len(), 6, "everything offered stays");
        let none = offered_slots(slots, &[]);
        assert_eq!(
            shape(&none),
            [
                format!("{zoom:?}"),
                "|".to_owned(),
                format!("{:?}", Command::Stage(StageCommand::TogglePlayback))
            ],
            "the rotate buttons go, with one of the two dividers around them"
        );
    }

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
    #[test]
    fn a_picture_that_can_be_saved_lists_crop_adjust_size_and_save_once_it_has_changes() {
        let picture = Stage::for_family(family_of(FormatKind::Raster), TextViews::default());
        let pdf = Stage::for_family(family_of(FormatKind::Pdf), TextViews::default());
        let words = |stage: &Stage, offer, tool| {
            picture_commands(stage, offer, tool)
                .iter()
                .map(Command::label)
                .collect::<Vec<_>>()
        };
        let pan = crate::Tool::Pan;
        assert_eq!(
            words(&picture, PictureOffer::Pristine, pan),
            ["Crop", "Adjust Size\u{2026}"]
        );
        assert_eq!(
            words(&picture, PictureOffer::Edited, pan),
            ["Crop", "Adjust Size\u{2026}", "Save"]
        );
        assert_eq!(
            words(&picture, PictureOffer::Edited, crate::Tool::Crop),
            ["Adjust Size\u{2026}", "Save"],
            "the tool already chosen is not offered"
        );
        assert!(words(&picture, PictureOffer::Unavailable, pan).is_empty());
        assert!(words(&pdf, PictureOffer::Edited, pan).is_empty());
    }
}
