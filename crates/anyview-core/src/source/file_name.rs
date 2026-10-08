//! `FileName`: the last component of a path, which is where sniffing reads the extension.

use crate::error::CoreError;

/// A file's own name, never a path: non-empty, not `.` or `..`, with no `/`, `\` or NUL. Names
/// that are not valid UTF-8 are read lossily.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileName(String);

impl FileName {
    /// `name`, or why it is not a file name.
    pub fn new(name: &str) -> Result<Self, CoreError> {
        let separator = |c: char| matches!(c, '/' | '\\' | '\0');
        if name.is_empty() || name == "." || name == ".." || name.contains(separator) {
            return Err(CoreError::NameInvalid {
                name: name.to_owned(),
            });
        }
        Ok(FileName(name.to_owned()))
    }

    /// The name of a file that has none (a root, or bytes nobody named).
    pub fn unnamed() -> Self {
        FileName("file".to_owned())
    }

    /// The whole name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text after the last dot, or `None` when there is no dot, the dot is the first
    /// character (`.bashrc` is a name, not an extension) or the dot is last. Case is kept.
    pub fn extension(&self) -> Option<&str> {
        let (stem, extension) = self.0.rsplit_once('.')?;
        (!stem.is_empty() && !extension.is_empty()).then_some(extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_with_a_separator_or_nothing_are_refused() {
        const CASES: &[(&str, &str)] = &[
            ("empty", ""),
            ("dot", "."),
            ("dotdot", ".."),
            ("slash", "a/b"),
            ("backslash", "a\\b"),
            ("nul", "a\0b"),
        ];
        for (name, input) in CASES {
            assert!(FileName::new(input).is_err(), "{name}");
        }
        assert!(FileName::new("a.b").is_ok());
    }

    #[test]
    fn the_extension_is_the_text_after_the_last_dot() {
        const CASES: &[(&str, &str, Option<&str>)] = &[
            ("simple", "photo.png", Some("png")),
            ("case kept", "PHOTO.PNG", Some("PNG")),
            ("compound", "a.tar.gz", Some("gz")),
            ("no dot", "Makefile", None),
            ("leading dot only", ".bashrc", None),
            ("trailing dot", "a.", None),
            ("leading dot and extension", ".a.txt", Some("txt")),
        ];
        for (name, input, want) in CASES {
            let file = FileName::new(input).unwrap();
            assert_eq!(file.extension(), *want, "{name}");
        }
    }
}
