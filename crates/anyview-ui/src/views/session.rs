//! What the window itself holds, as opposed to what a machine does: where a probe stands, the
//! document the load opened, the window of lines last read. These are the results of effects,
//! kept so the views can draw them and so a late result for a file the person left is dropped.

use crate::context::entries;
use crate::families::{LineWindow, LoadedDoc, family_of, views_of};
use crate::io::{NaturalSize, Probed};
use crate::sheet::ExportFacts;
use crate::{
    ChromeParams, Command, ContextParams, EditOffer, FileAccess, MediaOffer, Motion, PaletteParams,
    PanelParams, PlatformAbilities, PresentationParams, SheetParams, Spot, Stage, StageAbilities,
    StageCommand, StageParams, TextParams, TextViews, Ticket, TypedText, ViewerParams,
};
use anyview_core::{FileAction, FormatKind, Reach, actions_for, reach};
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
        exportable,
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
            ability.plays && playback == Playback::Playable
        }
        FileAction::RotateLeft
        | FileAction::RotateRight
        | FileAction::FlipHorizontal
        | FileAction::FlipVertical => {
            ability.edits.contains(&action) && edit != EditOffer::Withheld && writable
        }
        FileAction::RevertTo => !ability.edits.is_empty() && writable,
        FileAction::Export | FileAction::ConvertTo => ability.export.is_some() && exportable,
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
    /// Whether the Export sheet has anything to write: a recording with no export on offer and
    /// no package to name has none.
    pub exportable: bool,
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
        .filter(|command| offers.access == FileAccess::Writable || !edits_pages(**command))
        .filter(|command| stage.input_for(**command, params).is_some())
        .map(|command| Command::Stage(*command));
    let open = offers.platform.pick_files.then_some(Command::OpenFile);
    let pan = (offers.playback == Playback::Playable && matches!(stage, Stage::Raster(_)))
        .then_some(Command::UseTool(offers.tool.other()));
    open.into_iter()
        .chain(files)
        .chain(stages)
        .chain(pan)
        .collect()
}

/// Whether `command` changes the pages of the PDF it is run on.
fn edits_pages(command: StageCommand) -> bool {
    matches!(
        command,
        StageCommand::DeletePage | StageCommand::MovePageEarlier | StageCommand::MovePageLater
    )
}

/// The capsule's `slots` without the buttons for a file action the file does not take (`files` is
/// what the palette and the menu list), and without the dividers that leave: none first or last,
/// and never two together.
pub(super) fn offered_slots(
    slots: Vec<RankedSlot<Command>>,
    files: &[FileAction],
) -> Vec<RankedSlot<Command>> {
    let mut kept: Vec<RankedSlot<Command>> = Vec::with_capacity(slots.len());
    for ranked in slots {
        let taken = match &ranked.slot {
            CapsuleSlot::Item(item) => match item.value {
                Command::File(action) => files.contains(&action),
                Command::Stage(_)
                | Command::OpenFile
                | Command::UseTool(_)
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
        tool,
        marks,
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
    let exportable = match (stage, doc) {
        (Stage::Media(_), Some(doc)) => {
            let media = doc.view().media_offer();
            media.first().is_some() || media.needs().is_some()
        }
        _ => true,
    };
    let offers = Offers {
        playback,
        edit: offer,
        access,
        exportable,
        platform,
        tool,
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
            Command::Stage(_) | Command::OpenFile | Command::UseTool(_) | Command::Install(_) => {
                None
            }
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
pub(super) fn family(probed: &Probed) -> crate::StageFamily {
    family_of(probed.sniffed.kind())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextPick;
    use crate::families::family_of;
    use crate::panel::{PanelTab, PanelTabs};
    use crate::{ContextEntry, PlatformAbilities};
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
            exportable: true,
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
            exportable: false,
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
}
