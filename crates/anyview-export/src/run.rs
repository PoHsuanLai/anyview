//! Running an export, and making a printout.

use crate::choice::DocumentExport;
use crate::error::ExportError;
use crate::name::output_path;
use crate::produce::produce;
use crate::session::Session;
use crate::write::write_new;
use anyview_core::{
    ExportChoice, ExportExtension, ExportJob, FilePath, FormatKind, RasterExport, Sniffed, Source,
};
use std::path::Path;

/// Write `choice` for the file `source` (which sniffed as `sniffed`) beside it, under free
/// names: one file for most exports, one for each page for page images. Returns the files
/// written, in order. The original is never touched. If a file cannot be written, the ones this
/// export already wrote are removed. Blocking.
pub fn export(
    source: &Source,
    sniffed: &Sniffed,
    choice: DocumentExport,
) -> Result<Vec<FilePath>, ExportError> {
    let mut session = Session::default();
    let jobs = plan(&mut session, source.path(), sniffed, choice)?;
    if jobs.is_empty() {
        return Err(ExportError::NothingToWrite);
    }
    let extension = extension_of(choice);
    let mut written: Vec<FilePath> = Vec::with_capacity(jobs.len());
    for job in &jobs {
        match write_job(&mut session, source.path(), job, extension) {
            Ok(file) => written.push(file),
            Err(error) => {
                for file in &written {
                    let _gone = std::fs::remove_file(file.as_path());
                }
                return Err(error);
            }
        }
    }
    Ok(written)
}

/// The PDF a printer takes for the file `source`: a PDF is itself, and an image or a text
/// document is laid out on a page first. Blocking.
pub fn printout(source: &Source, sniffed: &Sniffed) -> Result<Vec<u8>, ExportError> {
    let file = source.path();
    let jobs = match sniffed.kind() {
        FormatKind::Pdf => return read(file.as_path()),
        FormatKind::Raster | FormatKind::Vector => {
            anyview_image::plan_export(file, RasterExport::Pdf)
        }
        FormatKind::Markdown | FormatKind::Code | FormatKind::PlainText => {
            anyview_text::plan_print(file, sniffed)
        }
        FormatKind::Video
        | FormatKind::Audio
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Book
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => Vec::new(),
    };
    match jobs.first() {
        Some(job) => produce(&mut Session::default(), job),
        None => Err(ExportError::NothingToWrite),
    }
}

/// The jobs `choice` is for `file`.
fn plan(
    session: &mut Session,
    file: &FilePath,
    sniffed: &Sniffed,
    choice: DocumentExport,
) -> Result<Vec<ExportJob>, ExportError> {
    Ok(match choice {
        DocumentExport::Raster(choice) => anyview_image::plan_export(file, choice),
        DocumentExport::Pdf(choice) => {
            let count = session.document(file)?.page_count();
            anyview_pdf::plan_export(file, choice, count)
        }
        DocumentExport::Text(choice) => anyview_text::plan_export(file, sniffed, choice),
    })
}

fn extension_of(choice: DocumentExport) -> ExportExtension {
    match choice {
        DocumentExport::Raster(choice) => choice.extension(),
        DocumentExport::Pdf(choice) => choice.extension(),
        DocumentExport::Text(choice) => choice.extension(),
    }
}

/// The file `job` writes, made and put beside `source`.
fn write_job(
    session: &mut Session,
    source: &FilePath,
    job: &ExportJob,
    extension: ExportExtension,
) -> Result<FilePath, ExportError> {
    let bytes = produce(session, job)?;
    let to =
        output_path(source.as_path(), job, extension).ok_or_else(|| ExportError::NoFreeName {
            path: source.as_path().to_path_buf(),
        })?;
    write_new(&bytes, &to)?;
    Ok(FilePath::new(&to)?)
}

fn read(path: &Path) -> Result<Vec<u8>, ExportError> {
    std::fs::read(path).map_err(|error| ExportError::Read {
        path: path.to_path_buf(),
        kind: error.kind(),
    })
}
