//! What every closed family of formats (raster, media container, font, archive, …) has in common:
//! the file extensions it is written with and its MIME type. One trait, one generic lookup.

use ds_core::word::Word;

/// A closed set of formats that share a kind of file and are told apart by extension and MIME.
pub(crate) trait Family: Word {
    /// The extensions the format is written with, lower-case and without the dot.
    fn extensions(self) -> &'static [&'static str];

    /// The format's MIME type.
    fn mime(self) -> &'static str;
}

/// The format of family `F` whose extension is `extension`, compared whole and ignoring case.
pub(crate) fn from_extension<F: Family>(extension: &str) -> Option<F> {
    F::ALL.iter().copied().find(|format| {
        format
            .extensions()
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::kind::Mime;

    /// A family's table holds well-formed extensions, valid MIME types and no extension twice.
    pub(crate) fn assert_well_formed<F: Family + std::fmt::Debug>() {
        let mut seen: Vec<&str> = Vec::new();
        for format in F::ALL {
            assert!(
                Mime::parse(format.mime()).is_ok(),
                "{format:?} has a bad MIME type"
            );
            assert!(
                !format.extensions().is_empty(),
                "{format:?} has no extension"
            );
            for extension in format.extensions() {
                assert_eq!(*extension, extension.to_ascii_lowercase(), "{format:?}");
                assert!(!extension.contains('.'), "{format:?}");
                assert!(!seen.contains(extension), "{extension} is listed twice");
                seen.push(extension);
            }
        }
    }
}
