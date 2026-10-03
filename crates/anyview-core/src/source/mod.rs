//! What the viewer was given: a path parsed once at the boundary, and the stamp that says which
//! version of the file was read.

mod file_name;
mod file_path;
mod stamp;

pub use file_name::FileName;
pub use file_path::FilePath;
pub use stamp::{ByteLen, FileStamp, ModTime};

/// A file the viewer was given (CLI, D-Bus, drop, launcher), parsed once at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Source {
    path: FilePath,
    stamp: FileStamp,
}

impl Source {
    /// The file at `path` as it stood at `stamp`.
    pub fn new(path: FilePath, stamp: FileStamp) -> Self {
        Source { path, stamp }
    }

    /// Where the file is.
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Which version of it was read.
    pub fn stamp(&self) -> FileStamp {
        self.stamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_round_trips_through_json() {
        let source = Source::new(
            FilePath::new("/docs/a.pdf").unwrap(),
            FileStamp {
                len: ByteLen(42),
                modified: ModTime(7),
            },
        );
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(serde_json::from_str::<Source>(&json).unwrap(), source);
        assert_eq!(source.path().as_path().to_str(), Some("/docs/a.pdf"));
        assert_eq!(source.stamp().len, ByteLen(42));
    }
}
