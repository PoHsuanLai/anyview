use super::commands::{commands, picture_commands};
use super::offers::{Exports, Offers, PictureOffer, Playback};
use super::rows::{names, offered_slots};
use crate::families::family_of;
use crate::{
    Command, ContextEntry, ContextPick, EditOffer, FileAccess, PanelTab, PanelTabs,
    PlatformAbilities, Stage, StageCommand, StageParams, TextParams, TextView, TextViews,
};
use FileAction::{
    ConvertTo, CopyFile, CopyPath, Export, FlipHorizontal, FlipVertical, PlayInBackground,
    PlayInMiniWindow, Print, RevertTo, RotateLeft, RotateRight, SaveCopy, Share, Trim,
};
use anyview_core::{FileAction, FormatKind};
use anyview_machines::seam::entries;
use ds::components::chrome::capsule::model::CapsuleSlot;
use ds::components::chrome::capsule::priority::RankedSlot;

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
    Trim,
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
        Archive, Audio, Book, Code, Folder, Font, Markdown, Office, Other, Pdf, PlainText, Raster,
        Table, Tree, Vector, Video,
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
            "a video plays in a small window, exports and trims",
            Video,
            with(&[Export, SaveCopy, RevertTo, Trim, PlayInMiniWindow]),
        ),
        (
            "a song plays in the background, exports, converts and trims",
            Audio,
            with(&[
                Export,
                ConvertTo,
                SaveCopy,
                RevertTo,
                Trim,
                PlayInBackground,
            ]),
        ),
        (
            "Markdown, code and text export, convert and print",
            Markdown,
            with(&[Export, ConvertTo, Print]),
        ),
        ("code", Code, with(&[Export, ConvertTo, Print])),
        ("plain text", PlainText, with(&[Export, ConvertTo, Print])),
        (
            "a table is edited in place: no export, but its versions",
            Table,
            with(&[SaveCopy, RevertTo]),
        ),
        (
            "a tree is edited in place: no export, but its versions",
            Tree,
            with(&[SaveCopy, RevertTo]),
        ),
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
        assert_eq!(row("Trim\u{2026}"), want.contains(&Trim), "{name}: menu");
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
            vec![PlayInMiniWindow, Trim],
        ),
        (
            "a recording that refuses a save",
            FormatKind::Audio,
            read_only,
            vec![Trim, RevertTo],
        ),
        (
            "a recording with no export to offer",
            FormatKind::Video,
            unexportable,
            vec![Export, ConvertTo, Trim],
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
            menu.iter()
                .any(|entry| matches!(entry, ContextEntry::Item { title: t, .. } if *t == title))
        };
        if offers.access == FileAccess::ReadOnly || offers.edit == EditOffer::Withheld {
            assert!(!in_menu("Rotate Left"), "{name}: menu");
        }
    }
}

#[test]
fn a_picture_with_no_writer_still_lists_what_turns_and_flips_it() {
    let copies = Offers {
        edit: EditOffer::Copies(anyview_core::RasterTarget::Png),
        ..plain()
    };
    let (palette, _) = lists(FormatKind::Raster, copies);
    for action in [RotateLeft, RotateRight, FlipHorizontal, FlipVertical] {
        assert!(palette.contains(&Command::File(action)), "{action:?}");
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
            scale: crate::ContentScale::ACTUAL,
        },
        edited: Edited::default(),
        find: None,
    });
    let params = |editable| {
        StageParams::default().with_text(TextParams {
            editable,
            ..TextParams::default()
        })
    };
    let locked = Offers {
        access: FileAccess::ReadOnly,
        ..plain()
    };
    const THREE: [StageCommand; 3] = [StageCommand::Edit, StageCommand::Done, StageCommand::Save];
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
