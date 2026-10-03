//! A file to open in a window, with the files around it for the arrow keys.

use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};

/// What a new window is told: its file, and the list ← and → walk when the folder could be read.
#[derive(Debug, Clone, PartialEq)]
pub struct Opening {
    /// The file.
    pub file: FilePath,
    /// The files beside it, with it among them.
    pub sequence: Option<Sequence>,
}

impl Opening {
    /// `file` with the files of its folder. Blocking: it lists the folder.
    pub fn around(file: FilePath) -> Opening {
        let sequence = sequence_around(&file);
        Opening { file, sequence }
    }
}

/// The visible regular files in `file`'s folder, by name, with `file` the one the sequence
/// points at; `None` when the folder cannot be read or `file` is not in it. Blocking.
pub fn sequence_around(file: &FilePath) -> Option<Sequence> {
    let folder = file.parent()?;
    let mut entries: Vec<FilePath> = std::fs::read_dir(folder.as_path())
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| FilePath::new(entry.path()).ok())
        .collect();
    entries.sort_by(|a, b| a.as_path().cmp(b.as_path()));
    Sequence::starting_at(
        NonEmpty::from_vec(entries)?,
        file,
        SequenceOrigin::Folder(folder),
    )
    .ok()
}
