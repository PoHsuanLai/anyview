//! Writing the person's files: an edit saved in place, a kept version put back, a copy. Blocking:
//! the desktop runs each on the blocking pool. A file is only ever written over through the
//! store's pipeline, which keeps the original first.

use super::outcome::Outcome;
use anyview_core::{FilePath, FormatKind};
use anyview_pdf::{PdfDocument, apply, page_op};
use anyview_store::{
    DEFAULT_KEEP, Durability, Pending, SavedAt, StoreError, VersionId, Versions, Written,
};
use anyview_ui::{EditRequest, Opened, TextSave, VersionKey, VersionRow};

/// Why an edit could not be made into the bytes of a new file.
#[derive(Debug, thiserror::Error)]
enum EditError {
    #[error("cannot read the file: {0}")]
    Read(std::io::Error),
    #[error("{0}")]
    Image(#[from] anyview_image::ImageError),
    #[error("{0}")]
    Pdf(#[from] anyview_pdf::PdfError),
    #[error("a file of this kind has no edit to save")]
    NoEdit,
}

/// Why a save or a restore did not write the file; the words are the person's.
#[derive(Debug, thiserror::Error)]
enum SaveError {
    #[error("cannot save the edit: {0}")]
    Edit(#[from] EditError),
    #[error("cannot keep the original before saving: {0}")]
    KeepOriginal(StoreError),
    #[error("cannot save the file: {0}")]
    Write(StoreError),
    #[error("cannot go back to that version: {0}")]
    Restore(StoreError),
}

/// The bytes `file` becomes under `request`.
fn edited(file: &Opened, request: EditRequest) -> Result<Vec<u8>, EditError> {
    let bytes = std::fs::read(file.source.path().as_path()).map_err(EditError::Read)?;
    match file.sniffed.kind() {
        FormatKind::Raster => Ok(anyview_image::edited(&bytes, &file.sniffed, request.edit)?),
        FormatKind::Pdf => {
            let document = PdfDocument::from_bytes(bytes)?;
            let op = page_op(request.edit, request.page)?;
            Ok(apply(&document, &[op])?)
        }
        FormatKind::Vector
        | FormatKind::PlainText
        | FormatKind::Markdown
        | FormatKind::Code
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Video
        | FormatKind::Audio
        | FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Book
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => Err(EditError::NoEdit),
    }
}

/// `file` saved in place as `request` changes it, its original kept first.
pub(super) fn save_edit(
    versions: &Versions,
    at: SavedAt,
    file: &Opened,
    request: EditRequest,
) -> Outcome {
    let path = file.source.path();
    let result = edited(file, request)
        .map_err(SaveError::from)
        .and_then(|bytes| written(versions, at, path, bytes));
    outcome_of(result, path)
}

/// `file` written over with the text the person edited, its original kept first.
pub(super) fn save_text(
    versions: &Versions,
    at: SavedAt,
    file: &Opened,
    save: TextSave,
) -> Outcome {
    let path = file.source.path();
    outcome_of(written(versions, at, path, save.into_bytes()), path)
}

/// `bytes` written over `path` once its original is kept.
fn written(
    versions: &Versions,
    at: SavedAt,
    path: &FilePath,
    bytes: Vec<u8>,
) -> Result<VersionId, SaveError> {
    let pending = Pending::new(path.as_path(), bytes);
    let backed_up = versions
        .back_up(pending, at)
        .map_err(SaveError::KeepOriginal)?;
    backed_up
        .write_in_place()
        .map(kept_of)
        .map_err(SaveError::Write)
}

/// The version a save kept. A save whose folder could not be synced is still a save: the bytes
/// are in the file, so it is logged and not reported as a failure.
fn kept_of(written: Written) -> VersionId {
    if written.durability == Durability::Unconfirmed {
        eprintln!(
            "anyview: saved {}, but its folder could not be synced to disk",
            written.target.display()
        );
    }
    written.kept
}

/// `version` put back as `path`; what the file is now is kept first.
pub(super) fn restore(
    versions: &Versions,
    at: SavedAt,
    path: &FilePath,
    version: &VersionId,
) -> Outcome {
    let result = versions
        .restore(version, at)
        .map(kept_of)
        .map_err(SaveError::Restore);
    outcome_of(result, path)
}

/// The kept version of `path` that `key` names, put back as the file.
pub(super) fn revert(
    versions: &Versions,
    at: SavedAt,
    path: &FilePath,
    key: &VersionKey,
) -> Outcome {
    let found = versions.list(path.as_path()).map(|listed| {
        listed
            .into_iter()
            .find(|version| version.id.to_string() == key.as_str())
    });
    match found {
        Ok(Some(version)) => restore(versions, at, path, &version.id),
        Ok(None) => Outcome::NotWritten("that version is no longer kept".to_owned()),
        Err(error) => Outcome::NotWritten(format!("cannot list the kept versions: {error}")),
    }
}

fn outcome_of(result: Result<VersionId, SaveError>, path: &FilePath) -> Outcome {
    match result {
        Ok(kept) => Outcome::Written {
            file: path.clone(),
            kept,
        },
        Err(why) => Outcome::NotWritten(why.to_string()),
    }
}

/// A copy of `file` at `to`, written whole or not at all and never over a file that is there:
/// the source is only read.
pub(super) fn save_copy(file: &FilePath, to: &FilePath) -> Outcome {
    let bytes = match std::fs::read(file.as_path()) {
        Ok(bytes) => bytes,
        Err(error) => return Outcome::Failed(format!("cannot read the file to copy: {error}")),
    };
    match anyview_export::write_new(&bytes, to.as_path()) {
        Ok(()) => Outcome::Wrote(to.clone()),
        Err(error) => Outcome::Failed(format!("cannot save the copy: {error}")),
    }
}
/// The versions kept of `path` as the sheet lists them, newest first. A store that cannot be
/// read lists none: the sheet then says there is nothing to go back to.
pub(super) fn rows_of(versions: &Versions, path: &FilePath) -> Vec<VersionRow> {
    versions
        .list(path.as_path())
        .unwrap_or_default()
        .into_iter()
        .map(|version| VersionRow {
            key: VersionKey::new(version.id.to_string()),
            saved_at: version.saved_at.0,
            size: version.size,
        })
        .collect()
}

/// The kept versions older than the default keep period before `at` are deleted.
pub fn prune_versions(versions: &Versions, at: SavedAt) -> Outcome {
    match versions.prune(DEFAULT_KEEP, at) {
        Ok(_removed) => Outcome::Done,
        Err(error) => Outcome::Failed(format!("cannot prune the kept versions: {error}")),
    }
}
