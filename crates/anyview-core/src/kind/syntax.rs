//! `SyntaxName`: a highlighter's name for a language. The list of languages is external data (the
//! highlighter's own set), so it is a parsed name and not an enum.

use crate::error::CoreError;
use std::borrow::Cow;

/// The name a highlighter knows a language by: lower-case ASCII letters, digits and `+ # - _ .`
/// (`rust`, `c++`, `c#`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SyntaxName(Cow<'static, str>);

impl SyntaxName {
    /// `text` as a syntax name, lower-cased, or why it is not one.
    pub fn new(text: &str) -> Result<Self, CoreError> {
        let lower = text.to_ascii_lowercase();
        let valid = !lower.is_empty()
            && lower
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+#-_.".contains(c));
        if valid {
            Ok(SyntaxName(Cow::Owned(lower)))
        } else {
            Err(CoreError::SyntaxNameInvalid {
                text: text.to_owned(),
            })
        }
    }

    /// A name from this crate's own tables, which a test checks parse.
    pub(crate) const fn known(text: &'static str) -> Self {
        SyntaxName(Cow::Borrowed(text))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SyntaxName {
    type Error = CoreError;

    fn try_from(text: String) -> Result<Self, CoreError> {
        SyntaxName::new(&text)
    }
}

impl From<SyntaxName> for String {
    fn from(name: SyntaxName) -> String {
        name.0.into_owned()
    }
}

/// Extensions of source and config files, each with the syntax that highlights it. Matched against
/// the whole extension, never a suffix.
const EXTENSIONS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("pyi", "python"),
    ("js", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("jsx", "javascript"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("c", "c"),
    ("h", "c"),
    ("cc", "c++"),
    ("cpp", "c++"),
    ("cxx", "c++"),
    ("hpp", "c++"),
    ("hh", "c++"),
    ("cs", "c#"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("go", "go"),
    ("rb", "ruby"),
    ("php", "php"),
    ("swift", "swift"),
    ("lua", "lua"),
    ("pl", "perl"),
    ("r", "r"),
    ("sh", "shell"),
    ("bash", "shell"),
    ("zsh", "shell"),
    ("fish", "shell"),
    ("sql", "sql"),
    ("html", "html"),
    ("htm", "html"),
    ("xml", "xml"),
    ("css", "css"),
    ("scss", "scss"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("toml", "toml"),
    ("ini", "ini"),
    ("conf", "ini"),
    ("diff", "diff"),
    ("patch", "diff"),
];

/// Whole file names (not extensions) of source files.
const NAMES: &[(&str, &str)] = &[("makefile", "makefile"), ("dockerfile", "dockerfile")];

/// The syntax that highlights files with `extension`, ignoring case.
pub(crate) fn for_extension(extension: &str) -> Option<SyntaxName> {
    EXTENSIONS
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(extension))
        .map(|(_, syntax)| SyntaxName::known(syntax))
}

/// The syntax that highlights a file called `name`, ignoring case.
pub(crate) fn for_name(name: &str) -> Option<SyntaxName> {
    NAMES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, syntax)| SyntaxName::known(syntax))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_names_are_lower_case_tokens() {
        const CASES: &[(&str, &str, Option<&str>)] = &[
            ("simple", "rust", Some("rust")),
            ("upper case", "Rust", Some("rust")),
            ("plus", "c++", Some("c++")),
            ("sharp", "c#", Some("c#")),
            ("empty", "", None),
            ("space", "objective c", None),
            ("slash", "a/b", None),
            ("non ascii", "rüst", None),
        ];
        for (name, text, want) in CASES {
            assert_eq!(
                SyntaxName::new(text).ok().as_ref().map(SyntaxName::as_str),
                *want,
                "{name}"
            );
        }
    }

    #[test]
    fn every_table_entry_is_a_valid_name_and_no_key_repeats() {
        let mut seen: Vec<&str> = Vec::new();
        for (key, syntax) in EXTENSIONS.iter().chain(NAMES) {
            assert!(SyntaxName::new(syntax).is_ok(), "{syntax}");
            assert_eq!(*key, key.to_ascii_lowercase(), "{key}");
            assert!(!seen.contains(key), "{key} is listed twice");
            seen.push(key);
        }
    }

    #[test]
    fn extensions_match_whole_and_ignoring_case() {
        const CASES: &[(&str, &str, Option<&str>)] = &[
            ("rust", "rs", Some("rust")),
            ("upper case", "RS", Some("rust")),
            ("a longer extension is not a suffix match", "xrs", None),
            ("a prefix of a known extension", "j", None),
            ("the tail of a name is not an extension", "s", None),
            ("unknown", "png", None),
        ];
        for (name, extension, want) in CASES {
            let got = for_extension(extension);
            assert_eq!(got.as_ref().map(SyntaxName::as_str), *want, "{name}");
        }
        assert_eq!(
            for_name("Makefile").as_ref().map(SyntaxName::as_str),
            Some("makefile")
        );
        assert_eq!(for_name("Makefile.bak"), None);
        assert_eq!(for_name("notmakefile"), None);
    }

    #[test]
    fn a_syntax_name_round_trips() {
        let name = SyntaxName::new("python").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"python\"");
        assert_eq!(serde_json::from_str::<SyntaxName>(&json).unwrap(), name);
        assert!(serde_json::from_str::<SyntaxName>("\"\"").is_err());
    }
}
