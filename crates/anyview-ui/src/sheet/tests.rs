use super::*;
use crate::keys::{Act, Press};
use crate::typed::TypedText;
use anyview_core::{
    ByteLen, FileAction, Helper, MetadataCarry, PageSelection, PdfExport, PdfExportKind,
    RasterExport, RasterExportKind, RasterTarget, Resize, TextExport,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;
use ds_core::vocab::{Shortcut, ShortcutKey};

const PNG: ExportDraft = ExportDraft::Raster(RasterExport::Image(
    RasterTarget::Png,
    Resize::Original,
    MetadataCarry::StripLocation,
));
const RASTER_PDF: ExportDraft = ExportDraft::Raster(RasterExport::Pdf);
const PDF_PAGES: ExportDraft = ExportDraft::Pdf(PdfExport::Pdf(PageSelection::All));
const PDF_TEXT: ExportDraft = ExportDraft::Pdf(PdfExport::PlainText);
const TEXT: ExportDraft = ExportDraft::Text(TextExport::PlainText);

const ROWS: &[VersionRow] = &[
    VersionRow {
        key: VersionKey::from_static("k/new"),
        saved_at: 200,
        size: ByteLen(20),
    },
    VersionRow {
        key: VersionKey::from_static("k/old"),
        saved_at: 100,
        size: ByteLen(10),
    },
];
const VERSIONS: VersionList = VersionList::from_static(ROWS);
const NEW: VersionKey = VersionKey::from_static("k/new");
const OLD: VersionKey = VersionKey::from_static("k/old");

const fn reverting(chosen: VersionKey) -> Sheet {
    Sheet::Revert {
        versions: VERSIONS,
        chosen,
    }
}
const fn copying(name: &'static str) -> Sheet {
    Sheet::SaveCopy {
        name: TypedText::from_static(name),
    }
}
const fn export(draft: ExportDraft) -> Sheet {
    Sheet::Export {
        draft,
        span: PageSpan::All,
    }
}
const fn rename(name: &'static str) -> Sheet {
    Sheet::Rename {
        name: TypedText::from_static(name),
    }
}

/// Name, state before, input, state after, outputs.
type Case = (&'static str, Sheet, SheetIn, Sheet, &'static [SheetOut]);

const CASES: &[Case] = &[
    (
        "opening the export sheet holds the format's default",
        Sheet::Closed,
        SheetIn::OpenExport(PNG),
        export(PNG),
        &[SheetOut::Opened],
    ),
    (
        "asking about the trash opens the confirmation",
        Sheet::Closed,
        SheetIn::AskTrash,
        Sheet::ConfirmTrash,
        &[SheetOut::Opened],
    ),
    (
        "asking to rename starts from the current name",
        Sheet::Closed,
        SheetIn::AskRename(TypedText::from_static("a.png")),
        rename("a.png"),
        &[SheetOut::Opened],
    ),
    (
        "a closed sheet ignores confirm",
        Sheet::Closed,
        SheetIn::Confirm,
        Sheet::Closed,
        &[],
    ),
    (
        "picking a kind of the same format resets its options",
        export(PNG),
        SheetIn::PickKind(ExportKindPick::Raster(RasterExportKind::Pdf)),
        export(RASTER_PDF),
        &[],
    ),
    (
        "picking a kind of another format is refused",
        export(PNG),
        SheetIn::PickKind(ExportKindPick::Pdf(PdfExportKind::Markdown)),
        export(PNG),
        &[],
    ),
    (
        "changing an option within the format replaces the draft",
        export(PDF_PAGES),
        SheetIn::Change(PDF_TEXT),
        export(PDF_TEXT),
        &[],
    ),
    (
        "changing to a draft of another format is refused",
        export(PDF_PAGES),
        SheetIn::Change(TEXT),
        export(PDF_PAGES),
        &[],
    ),
    (
        "confirming exports the draft and closes",
        export(PDF_TEXT),
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Export(PDF_TEXT), SheetOut::Closed],
    ),
    (
        "cancelling closes without exporting",
        export(PDF_TEXT),
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "a sheet that is up will not open another",
        export(PNG),
        SheetIn::AskTrash,
        export(PNG),
        &[],
    ),
    (
        "confirming the trash trashes and closes",
        Sheet::ConfirmTrash,
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Trash, SheetOut::Closed],
    ),
    (
        "cancelling the trash closes without trashing",
        Sheet::ConfirmTrash,
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "typing replaces the new name",
        rename("a"),
        SheetIn::Typed(TypedText::from_static("ab")),
        rename("ab"),
        &[],
    ),
    (
        "confirming a name renames and closes",
        rename("b.png"),
        SheetIn::Confirm,
        Sheet::Closed,
        &[
            SheetOut::Rename(TypedText::from_static("b.png")),
            SheetOut::Closed,
        ],
    ),
    (
        "an empty name cannot be confirmed",
        rename(""),
        SheetIn::Confirm,
        rename(""),
        &[],
    ),
    (
        "asking to save a copy starts from the proposed name",
        Sheet::Closed,
        SheetIn::AskSaveCopy(TypedText::from_static("a copy.png")),
        copying("a copy.png"),
        &[SheetOut::Opened],
    ),
    (
        "typing replaces the name of the copy",
        copying("a"),
        SheetIn::Typed(TypedText::from_static("ab")),
        copying("ab"),
        &[],
    ),
    (
        "confirming a name saves the copy and closes",
        copying("b.png"),
        SheetIn::Confirm,
        Sheet::Closed,
        &[
            SheetOut::SaveCopy(TypedText::from_static("b.png")),
            SheetOut::Closed,
        ],
    ),
    (
        "a copy with no name cannot be confirmed",
        copying(""),
        SheetIn::Confirm,
        copying(""),
        &[],
    ),
    (
        "cancelling a copy closes",
        copying("b.png"),
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "the revert sheet opens on the newest version",
        Sheet::Closed,
        SheetIn::OpenRevert(Some(VERSIONS)),
        reverting(NEW),
        &[SheetOut::Opened],
    ),
    (
        "a file with no kept version opens the sheet that says so",
        Sheet::Closed,
        SheetIn::OpenRevert(None),
        Sheet::NoVersions,
        &[SheetOut::Opened],
    ),
    (
        "picking a row chooses it",
        reverting(NEW),
        SheetIn::PickVersion(OLD),
        reverting(OLD),
        &[],
    ),
    (
        "a version that is not listed cannot be picked",
        reverting(NEW),
        SheetIn::PickVersion(VersionKey::from_static("k/none")),
        reverting(NEW),
        &[],
    ),
    (
        "confirming goes back to the chosen version and closes",
        reverting(OLD),
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Revert(OLD), SheetOut::Closed],
    ),
    (
        "cancelling the revert closes without going back",
        reverting(OLD),
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "enter puts away the sheet that says there is nothing to go back to",
        Sheet::NoVersions,
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "that sheet ignores a request to open another",
        Sheet::NoVersions,
        SheetIn::AskTrash,
        Sheet::NoVersions,
        &[],
    ),
    (
        "cancelling a rename closes",
        rename("b.png"),
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "leaving with changes unsaved asks what to do with them",
        Sheet::Closed,
        SheetIn::AskUnsaved(Departure::Close),
        unsaved(Departure::Close),
        &[SheetOut::Opened],
    ),
    (
        "Save writes the changes and then goes on",
        unsaved(Departure::Close),
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Save(Departure::Close), SheetOut::Closed],
    ),
    (
        "Don't Save lets the changes go and goes on",
        unsaved(Departure::Finish),
        SheetIn::Discard,
        Sheet::Closed,
        &[SheetOut::Discard(Departure::Finish), SheetOut::Closed],
    ),
    (
        "Cancel keeps the person where they were",
        unsaved(Departure::Close),
        SheetIn::Cancel,
        Sheet::Closed,
        &[SheetOut::Closed],
    ),
    (
        "replacing another program's changes writes the text",
        Sheet::ConfirmReplace,
        SheetIn::Confirm,
        Sheet::Closed,
        &[SheetOut::Replace, SheetOut::Closed],
    ),
    (
        "that question has no Don't Save",
        Sheet::ConfirmReplace,
        SheetIn::Discard,
        Sheet::ConfirmReplace,
        &[],
    ),
    (
        "a sheet that is up will not open the question",
        Sheet::ConfirmTrash,
        SheetIn::AskUnsaved(Departure::Close),
        Sheet::ConfirmTrash,
        &[],
    ),
];

const fn unsaved(departure: Departure) -> Sheet {
    Sheet::Unsaved(departure)
}

#[test]
fn every_row_of_the_table_steps_as_written() {
    for (name, from, input, state, outs) in CASES {
        let (next, out) = from
            .clone()
            .step(input.clone(), Stamp(0), &SheetParams::default(), &());
        assert_eq!(next, *state, "{name}: state");
        assert_eq!(out.as_slice(), *outs, "{name}: outputs");
        assert_eq!(next.wake(), None, "{name}: a sheet keeps no timer");
    }
}

#[test]
fn a_draft_knows_its_format() {
    assert_eq!(PNG.family(), ExportFamily::Raster);
    assert_eq!(PDF_TEXT.family(), ExportFamily::Pdf);
    assert_eq!(TEXT.family(), ExportFamily::Text);
}

#[test]
fn enter_confirms_and_escape_cancels_and_duplicates_chord_discards() {
    let key = |key| Press::Key(Shortcut(vec![key]));
    assert_eq!(
        SheetIn::from_press(&key(ShortcutKey::Enter)),
        Some(SheetIn::Confirm)
    );
    assert_eq!(
        SheetIn::from_press(&key(ShortcutKey::Escape)),
        Some(SheetIn::Cancel)
    );
    assert_eq!(SheetIn::from_press(&key(ShortcutKey::Char('x'))), None);
    assert_eq!(
        SheetIn::from_press(&Press::Act(Act::File(FileAction::Duplicate))),
        Some(SheetIn::Discard)
    );
    assert_eq!(SheetIn::from_press(&Press::Act(Act::Palette)), None);
}

#[test]
fn the_sheet_that_names_a_missing_package_is_put_away_by_enter_or_escape_and_writes_nothing() {
    let needs = anyview_core::Fact::new(
        anyview_core::FactLabel::Needs,
        anyview_core::FactValue::text("anyview-ffmpeg (to convert it)"),
    );
    let (open, outs) = Sheet::Closed.step(
        SheetIn::OpenUnavailable(needs.clone(), None),
        Stamp(0),
        &SheetParams::default(),
        &(),
    );
    assert_eq!(
        open,
        Sheet::Unavailable {
            needs: needs.clone(),
            helper: None,
        }
    );
    assert_eq!(outs, [SheetOut::Opened]);
    for (name, input) in [("Enter", SheetIn::Confirm), ("Escape", SheetIn::Cancel)] {
        let (closed, outs) = open
            .clone()
            .step(input, Stamp(0), &SheetParams::default(), &());
        assert_eq!(closed, Sheet::Closed, "{name}");
        assert_eq!(outs, [SheetOut::Closed], "{name}: no export is written");
    }
    let (still, outs) = open.clone().step(
        SheetIn::OpenExport(PNG),
        Stamp(0),
        &SheetParams::default(),
        &(),
    );
    assert_eq!(still, open, "a sheet that is up ignores another");
    assert!(outs.is_empty());
}

fn helping(phase: HelperPhase) -> Sheet {
    Sheet::Helper {
        helper: Helper::HeicDecode,
        phase,
    }
}

#[test]
fn the_install_sheet_asks_installs_and_follows_how_it_ended() {
    use HelperEnd as End;
    use HelperPhase::{Ask, Failed, Installing, NotFound, Unsupported};
    let heic = Helper::HeicDecode;
    // name, state before, input, state after, outputs
    type Row = (&'static str, Sheet, SheetIn, Sheet, Vec<SheetOut>);
    let rows: Vec<Row> = vec![
        (
            "offering opens the question",
            Sheet::Closed,
            SheetIn::OfferHelper(heic),
            helping(Ask),
            vec![SheetOut::Opened],
        ),
        (
            "Install asks the host and waits",
            helping(Ask),
            SheetIn::Confirm,
            helping(Installing),
            vec![SheetOut::Provide(heic)],
        ),
        (
            "Not Now closes and asks nothing",
            helping(Ask),
            SheetIn::Cancel,
            Sheet::Closed,
            vec![SheetOut::Closed],
        ),
        (
            "Return and Escape do nothing while the system installs",
            helping(Installing),
            SheetIn::Confirm,
            helping(Installing),
            vec![],
        ),
        (
            "Escape does nothing while the system installs",
            helping(Installing),
            SheetIn::Cancel,
            helping(Installing),
            vec![],
        ),
        (
            "an install that worked closes the sheet and opens the file again",
            helping(Installing),
            SheetIn::HelperEnded(heic, End::Installed),
            Sheet::Closed,
            vec![SheetOut::Reopen, SheetOut::Closed],
        ),
        (
            "a tool that arrived by itself closes the question the same way",
            helping(Ask),
            SheetIn::HelperEnded(heic, End::Installed),
            Sheet::Closed,
            vec![SheetOut::Reopen, SheetOut::Closed],
        ),
        (
            "a no at the password prompt closes quietly",
            helping(Installing),
            SheetIn::HelperEnded(heic, End::Declined),
            Sheet::Closed,
            vec![SheetOut::Closed],
        ),
        (
            "no package is a phase of its own",
            helping(Installing),
            SheetIn::HelperEnded(heic, End::NotFound("libheif-tools".to_owned())),
            helping(NotFound("libheif-tools".to_owned())),
            vec![],
        ),
        (
            "no way to install is a phase of its own",
            helping(Installing),
            SheetIn::HelperEnded(heic, End::Unsupported("heif-dec".to_owned())),
            helping(Unsupported("heif-dec".to_owned())),
            vec![],
        ),
        (
            "a failure keeps the package manager's words",
            helping(Installing),
            SheetIn::HelperEnded(heic, End::Failed("No network.".to_owned())),
            helping(Failed("No network.".to_owned())),
            vec![],
        ),
        (
            "an answer about another tool is not this sheet's",
            helping(Installing),
            SheetIn::HelperEnded(Helper::RawDecode, End::Installed),
            helping(Installing),
            vec![],
        ),
        (
            "Return closes a failure",
            helping(Failed("x".to_owned())),
            SheetIn::Confirm,
            Sheet::Closed,
            vec![SheetOut::Closed],
        ),
        (
            "Escape closes no package",
            helping(NotFound("libheif-tools".to_owned())),
            SheetIn::Cancel,
            Sheet::Closed,
            vec![SheetOut::Closed],
        ),
        (
            "Return closes no way to install",
            helping(Unsupported("heif-dec".to_owned())),
            SheetIn::Confirm,
            Sheet::Closed,
            vec![SheetOut::Closed],
        ),
        (
            "an answer for a sheet that is not up is ignored",
            Sheet::Closed,
            SheetIn::HelperEnded(heic, End::Installed),
            Sheet::Closed,
            vec![],
        ),
        (
            "a sheet that is up ignores a request for another",
            helping(Ask),
            SheetIn::AskTrash,
            helping(Ask),
            vec![],
        ),
        (
            "another sheet ignores an offer",
            Sheet::ConfirmTrash,
            SheetIn::OfferHelper(heic),
            Sheet::ConfirmTrash,
            vec![],
        ),
    ];
    for (name, before, input, after, outs) in rows {
        let (now, out) = before.step(input, Stamp(0), &SheetParams::default(), &());
        assert_eq!(now, after, "{name}");
        assert_eq!(out, outs, "{name}");
    }
}

mod tuning;
