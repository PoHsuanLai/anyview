//! `FilePath`: an absolute, lexically normalised path.

use crate::error::CoreError;
use crate::source::FileName;
use std::path::{Component, Path, PathBuf};

/// An absolute path with `.` and `..` resolved lexically (no file system access, so symlinks are
/// not followed) and repeated separators collapsed. `..` at the root stays at the root.
///
/// Stored in view history, so it deserialises through [`FilePath::new`] and a relative path in a
/// stored file is refused at load. A path that is not valid UTF-8 cannot be written to JSON.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct FilePath(PathBuf);

impl FilePath {
    /// `path` made absolute-normal, or why it is not absolute.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(CoreError::PathNotAbsolute {
                path: path.to_path_buf(),
            });
        }
        Ok(FilePath(normalised(path)))
    }

    /// The normalised path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The last component, or `None` for a root.
    pub fn file_name(&self) -> Option<FileName> {
        let name = self.0.file_name()?;
        FileName::new(&name.to_string_lossy()).ok()
    }

    /// The directory holding this path, or `None` for a root.
    pub fn parent(&self) -> Option<FilePath> {
        self.0.parent().map(|parent| FilePath(parent.to_path_buf()))
    }
}

impl TryFrom<PathBuf> for FilePath {
    type Error = CoreError;

    fn try_from(path: PathBuf) -> Result<Self, CoreError> {
        FilePath::new(path)
    }
}

impl From<FilePath> for PathBuf {
    fn from(path: FilePath) -> PathBuf {
        path.0
    }
}

/// `path` with `.` dropped, `..` resolved against what precedes it, and `..` at a root dropped.
fn normalised(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let at_root = out.parent().is_none();
                if !at_root {
                    out.pop();
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_normalise_lexically() {
        const CASES: &[(&str, &str, &str)] = &[
            ("plain", "/a/b/c.png", "/a/b/c.png"),
            ("dot", "/a/./b", "/a/b"),
            ("dotdot", "/a/b/../c", "/a/c"),
            ("double separator", "/a//b", "/a/b"),
            ("trailing separator", "/a/b/", "/a/b"),
            ("dotdot at root", "/../a", "/a"),
            ("dotdot past root", "/a/../../b", "/b"),
            ("root", "/", "/"),
        ];
        for (name, input, want) in CASES {
            let got = FilePath::new(input).map(|p| p.as_path().to_path_buf());
            assert_eq!(got, Ok(PathBuf::from(want)), "{name}");
        }
    }

    #[test]
    fn a_relative_path_is_refused() {
        const CASES: &[(&str, &str)] = &[("bare", "a.png"), ("dot", "./a.png"), ("up", "../a.png")];
        for (name, input) in CASES {
            assert_eq!(
                FilePath::new(input),
                Err(CoreError::PathNotAbsolute {
                    path: PathBuf::from(input)
                }),
                "{name}"
            );
        }
    }

    #[test]
    fn file_name_and_parent_split_the_path() {
        let path = FilePath::new("/photos/2024/cat.jpg").unwrap();
        assert_eq!(
            path.file_name().map(|n| n.as_str().to_owned()),
            Some("cat.jpg".to_owned())
        );
        assert_eq!(path.parent(), FilePath::new("/photos/2024").ok());
        let root = FilePath::new("/").unwrap();
        assert_eq!(root.file_name(), None);
        assert_eq!(root.parent(), None);
    }

    #[test]
    fn serde_round_trips_and_refuses_a_relative_path() {
        let path = FilePath::new("/a/b.txt").unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"/a/b.txt\"");
        assert_eq!(serde_json::from_str::<FilePath>(&json).unwrap(), path);
        assert!(serde_json::from_str::<FilePath>("\"b.txt\"").is_err());
    }
}
