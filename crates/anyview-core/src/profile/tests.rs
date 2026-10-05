//! Table tests for the per-kind profile: `actions_for`, `edits_for` and `stage_support`.

use super::{actions_for, edits_for, mime_for, stage_support};
use crate::action::{FileAction, Reach, reach};
use crate::edit::EditKind;
use crate::kind::FormatKind;
use crate::peek::StageSupport;
use ds_core::word::Word;

const COMMON: &[FileAction] = &[
    FileAction::Open,
    FileAction::OpenWith,
    FileAction::RevealInFolder,
    FileAction::CopyFile,
    FileAction::CopyPath,
    FileAction::Share,
    FileAction::Rename,
    FileAction::Duplicate,
    FileAction::MoveToTrash,
];

/// A kind's extras: what it adds after the common actions, its edits and its stage.
struct Row {
    kind: FormatKind,
    extras: &'static [FileAction],
    edits: &'static [EditKind],
    stage: StageSupport,
}

const fn row(
    kind: FormatKind,
    extras: &'static [FileAction],
    edits: &'static [EditKind],
    stage: StageSupport,
) -> Row {
    Row {
        kind,
        extras,
        edits,
        stage,
    }
}

const CASES: &[Row] = &[
    row(
        FormatKind::Pdf,
        &[
            FileAction::Print,
            FileAction::Export,
            FileAction::ConvertTo,
            FileAction::SaveCopy,
            FileAction::RevertTo,
            FileAction::RotateLeft,
            FileAction::RotateRight,
        ],
        &[EditKind::Rotate, EditKind::DeletePages, EditKind::MovePage],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Raster,
        &[
            FileAction::Print,
            FileAction::Export,
            FileAction::ConvertTo,
            FileAction::SaveCopy,
            FileAction::RevertTo,
            FileAction::RotateLeft,
            FileAction::RotateRight,
            FileAction::FlipHorizontal,
            FileAction::FlipVertical,
        ],
        &[EditKind::Rotate, EditKind::Flip],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Vector,
        &[FileAction::Print, FileAction::Export],
        &[],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Video,
        &[FileAction::Export, FileAction::PlayInMiniWindow],
        &[],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Audio,
        &[
            FileAction::Export,
            FileAction::ConvertTo,
            FileAction::PlayInBackground,
        ],
        &[],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Markdown,
        &[FileAction::Print, FileAction::Export, FileAction::ConvertTo],
        &[],
        StageSupport::Stage,
    ),
    row(
        FormatKind::Code,
        &[FileAction::Print, FileAction::Export, FileAction::ConvertTo],
        &[],
        StageSupport::Stage,
    ),
    row(
        FormatKind::PlainText,
        &[FileAction::Print, FileAction::Export, FileAction::ConvertTo],
        &[],
        StageSupport::Stage,
    ),
    row(FormatKind::Table, &[], &[], StageSupport::Stage),
    row(FormatKind::Tree, &[], &[], StageSupport::Stage),
    row(FormatKind::Font, &[], &[], StageSupport::PeekOnly),
    row(FormatKind::Archive, &[], &[], StageSupport::PeekOnly),
    row(FormatKind::Book, &[], &[], StageSupport::PeekOnly),
    row(FormatKind::Office, &[], &[], StageSupport::PeekOnly),
    row(FormatKind::Other, &[], &[], StageSupport::PeekOnly),
];

const FOLDER_ACTIONS: &[FileAction] = &[
    FileAction::Open,
    FileAction::RevealInFolder,
    FileAction::CopyFile,
    FileAction::CopyPath,
    FileAction::Share,
    FileAction::Rename,
    FileAction::Duplicate,
    FileAction::MoveToTrash,
];

#[test]
fn each_kind_offers_the_common_actions_then_its_own() {
    for case in CASES {
        let want: Vec<FileAction> = COMMON.iter().chain(case.extras).copied().collect();
        assert_eq!(
            actions_for(case.kind),
            want.as_slice(),
            "{:?} actions",
            case.kind
        );
        assert_eq!(edits_for(case.kind), case.edits, "{:?} edits", case.kind);
        assert_eq!(
            stage_support(case.kind),
            case.stage,
            "{:?} stage",
            case.kind
        );
    }
}

#[test]
fn a_folder_has_no_open_with_and_no_stage() {
    assert_eq!(actions_for(FormatKind::Folder), FOLDER_ACTIONS);
    assert_eq!(edits_for(FormatKind::Folder), &[] as &[EditKind]);
    assert_eq!(stage_support(FormatKind::Folder), StageSupport::PeekOnly);
}

#[test]
fn the_table_names_every_kind_once() {
    let mut named: Vec<FormatKind> = CASES.iter().map(|case| case.kind).collect();
    named.push(FormatKind::Folder);
    for kind in FormatKind::ALL {
        assert_eq!(named.iter().filter(|k| *k == kind).count(), 1, "{kind:?}");
    }
}

#[test]
fn every_kind_lists_each_action_once_and_opens_first() {
    for kind in FormatKind::ALL {
        let actions = actions_for(*kind);
        assert_eq!(actions.first(), Some(&FileAction::Open), "{kind:?}");
        for action in actions {
            assert_eq!(
                actions.iter().filter(|a| *a == action).count(),
                1,
                "{kind:?} {action:?}"
            );
        }
    }
}

#[test]
fn edit_actions_appear_exactly_when_the_edit_is_offered() {
    // name, the action, the edit that must be offered for it
    const EDIT_ACTIONS: &[(FileAction, EditKind)] = &[
        (FileAction::RotateLeft, EditKind::Rotate),
        (FileAction::RotateRight, EditKind::Rotate),
        (FileAction::FlipHorizontal, EditKind::Flip),
        (FileAction::FlipVertical, EditKind::Flip),
    ];
    for kind in FormatKind::ALL {
        let actions = actions_for(*kind);
        let edits = edits_for(*kind);
        for (action, edit) in EDIT_ACTIONS {
            assert_eq!(
                actions.contains(action),
                edits.contains(edit),
                "{kind:?} {action:?}"
            );
        }
        let editable = !edits.is_empty();
        assert_eq!(
            actions.contains(&FileAction::RevertTo),
            editable,
            "{kind:?} revert"
        );
        assert_eq!(
            actions.contains(&FileAction::SaveCopy),
            editable,
            "{kind:?} save a copy"
        );
    }
}

#[test]
fn a_launcher_row_never_lists_a_viewer_only_action_it_cannot_run() {
    // Whatever a kind offers, the viewer-only ones need the content open: `Reach` says so, and the
    // launcher filters by it. Every kind must still leave the launcher at least Open.
    for kind in FormatKind::ALL {
        let launcher: Vec<FileAction> = actions_for(*kind)
            .iter()
            .copied()
            .filter(|action| reach(*action) != Reach::Viewer)
            .collect();
        assert!(launcher.contains(&FileAction::Open), "{kind:?}");
        assert!(!launcher.contains(&FileAction::Print), "{kind:?}");
    }
}

#[test]
fn every_kind_has_a_media_type_that_names_it_back() {
    for kind in FormatKind::ALL {
        let mime = mime_for(*kind);
        // Code and plain text share `text/plain`, which names plain text.
        let want = if *kind == FormatKind::Code {
            FormatKind::PlainText
        } else {
            *kind
        };
        assert_eq!(crate::kind_of_mime(&mime), want, "{kind:?} gave {mime:?}");
    }
}
