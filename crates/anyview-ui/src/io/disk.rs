//! The files a Markdown document refers to, read from the disk (the edge's `LocalFiles`).

use anyview_core::FilePath;
use anyview_text::LocalFiles;

/// Reads local images off the disk for a document being rendered.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DiskFiles;

impl LocalFiles for DiskFiles {
    fn read(&self, path: &FilePath) -> Option<Vec<u8>> {
        std::fs::read(path.as_path()).ok()
    }
}
