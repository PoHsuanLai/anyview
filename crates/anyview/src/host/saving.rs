//! Writing the person's files: an edit saved in place, a kept version put back, a copy. Blocking:
//! the desktop runs each on the blocking pool. A file is only ever written over through the
//! store's pipeline, which keeps the original first.

use super::outcome::Outcome;
use anyview_core::{FilePath, FormatKind};
use anyview_pdf::{PdfDocument, apply, page_op};
use anyview_store::{DEFAULT_KEEP, Pending, SavedAt, VersionId, Versions};
use anyview_ui::{EditRequest, Probed, VersionKey, VersionRow};
use std::path::{Path, PathBuf};

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

/// The bytes `file` becomes under `request`.
fn edited(file: &Probed, request: EditRequest) -> Result<Vec<u8>, EditError> {
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
    file: &Probed,
    request: EditRequest,
) -> Outcome {
    let path = file.source.path();
    let result = edited(file, request)
        .map_err(|error| format!("cannot save the edit: {error}"))
        .and_then(|bytes| written(versions, at, path, bytes));
    outcome_of(result, path)
}

/// `bytes` written over `path` once its original is kept.
fn written(
    versions: &Versions,
    at: SavedAt,
    path: &FilePath,
    bytes: Vec<u8>,
) -> Result<VersionId, String> {
    let pending = Pending::new(path.as_path(), bytes);
    let backed_up = versions
        .back_up(pending, at)
        .map_err(|error| format!("cannot keep the original before saving: {error}"))?;
    backed_up
        .write_in_place()
        .map(|written| written.kept)
        .map_err(|error| format!("cannot save the file: {error}"))
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
        .map(|written| written.kept)
        .map_err(|error| format!("cannot go back to that version: {error}"));
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

fn outcome_of(result: Result<VersionId, String>, path: &FilePath) -> Outcome {
    match result {
        Ok(kept) => Outcome::Written {
            file: path.clone(),
            kept,
        },
        Err(why) => Outcome::NotWritten(why),
    }
}

/// A copy of `file` at `to`: written beside it under a temporary name and moved into place, so
/// the destination is whole or absent. A destination that exists is never replaced, and the
/// source is only read.
pub(super) fn save_copy(file: &FilePath, to: &FilePath) -> Outcome {
    let (source, target) = (file.as_path(), to.as_path());
    if target.exists() {
        return Outcome::Failed(format!(
            "cannot save the copy: {} already exists",
            target.display()
        ));
    }
    let temp = temp_beside(target);
    let copied = std::fs::copy(source, &temp).and_then(|_| {
        if target.exists() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        std::fs::rename(&temp, target)
    });
    match copied {
        Ok(()) => Outcome::Done,
        Err(error) => {
            let _gone = std::fs::remove_file(&temp);
            Outcome::Failed(format!("cannot save the copy: {error}"))
        }
    }
}

/// The name the copy has until it is whole.
fn temp_beside(target: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(target.file_name().unwrap_or_default());
    name.push(format!(".anyview-{}.tmp", std::process::id()));
    target.with_file_name(name)
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
