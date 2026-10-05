//! Images in a Markdown document. Quire's sealed frames load only `data:` URLs, so a local image
//! is read through an injected reader and written into the HTML as one.

use super::links::scheme;
use anyview_core::{FileHead, FileName, FilePath, FormatKind, SniffStep, sniff};

/// The largest image, in bytes, that is inlined: a `data:` URL is a third larger than the file and
/// lives in the HTML string.
const MAX_INLINE: usize = 8 * 1024 * 1024;

/// Reads the files a document refers to. The viewer's edge implements it over the disk and a test
/// over a map, so rendering stays pure.
pub trait LocalFiles {
    /// The bytes of the file at `path`, or `None` when it cannot be read.
    fn read(&self, path: &FilePath) -> Option<Vec<u8>>;
}

/// No files at all: every local image is shown as its alt text. A peek renders with this.
#[derive(Debug, Clone, Copy)]
pub struct NoFiles;

impl LocalFiles for NoFiles {
    fn read(&self, _: &FilePath) -> Option<Vec<u8>> {
        None
    }
}

/// The files of the disk: a document's images are read from where it lies. The edge's reader.
#[derive(Debug, Clone, Copy)]
pub struct DiskFiles;

impl LocalFiles for DiskFiles {
    fn read(&self, path: &FilePath) -> Option<Vec<u8>> {
        std::fs::read(path.as_path()).ok()
    }
}

/// `%20` and its kind decoded, bytes that are not valid percent escapes left as they are.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The file an image reference names: the reference without its query and fragment, percent
/// decoded, joined to `base` (the document's directory) unless it is absolute.
fn target(url: &str, base: Option<&FilePath>) -> Option<FilePath> {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    let decoded = percent_decode(&url[..end]);
    if decoded.is_empty() {
        return None;
    }
    if decoded.starts_with('/') {
        return FilePath::new(&decoded).ok();
    }
    FilePath::new(base?.as_path().join(decoded)).ok()
}

/// The `data:` URL an image reference becomes, or `None` when it must not be shown: a remote or
/// otherwise non-local address, a file that cannot be read, is too large or is not an image.
/// A reference that already is a `data:image/` URL is kept.
pub(super) fn data_url(
    url: &str,
    base: Option<&FilePath>,
    files: &dyn LocalFiles,
) -> Option<String> {
    let url = url.trim();
    match scheme(url).as_deref() {
        Some("data") => return url.starts_with("data:image/").then(|| url.to_owned()),
        Some(_) => return None,
        None => {}
    }
    let path = target(url, base)?;
    let bytes = files.read(&path)?;
    if bytes.len() > MAX_INLINE {
        return None;
    }
    let name = path.file_name()?;
    let mime = image_mime(&bytes, &name)?;
    Some(format!(
        "data:{mime};base64,{}",
        ds_core::base64::encode(&bytes)
    ))
}

/// The MIME type of an image file, or `None` for a file that sniffs as anything else.
fn image_mime(bytes: &[u8], name: &FileName) -> Option<String> {
    let head = FileHead::new(&bytes[..bytes.len().min(4096)]);
    match sniff(&head, name) {
        SniffStep::Done(sniffed)
            if matches!(sniffed.kind(), FormatKind::Raster | FormatKind::Vector) =>
        {
            Some(sniffed.mime().as_str().to_owned())
        }
        SniffStep::Done(_) | SniffStep::LookInside(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A document directory held in a map.
    struct Files(HashMap<String, Vec<u8>>);

    impl LocalFiles for Files {
        fn read(&self, path: &FilePath) -> Option<Vec<u8>> {
            self.0.get(path.as_path().to_str()?).cloned()
        }
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
    const SVG: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect/></svg>";

    fn files() -> Files {
        Files(HashMap::from([
            ("/notes/pic.png".to_owned(), PNG.to_vec()),
            ("/notes/my pic.png".to_owned(), PNG.to_vec()),
            ("/notes/img/logo.svg".to_owned(), SVG.to_vec()),
            ("/shared/up.png".to_owned(), PNG.to_vec()),
            ("/notes/readme.txt".to_owned(), b"words".to_vec()),
            (
                "/notes/huge.png".to_owned(),
                [PNG, &vec![0; MAX_INLINE]].concat(),
            ),
        ]))
    }

    fn png_url() -> String {
        format!("data:image/png;base64,{}", ds_core::base64::encode(PNG))
    }

    #[test]
    fn local_images_become_data_urls_and_everything_else_is_refused() {
        let base = FilePath::new("/notes").unwrap();
        let svg_url = format!("data:image/svg+xml;base64,{}", ds_core::base64::encode(SVG));
        // name, reference, result
        let cases: Vec<(&str, &str, Option<String>)> = vec![
            ("relative", "pic.png", Some(png_url())),
            ("dot slash", "./pic.png", Some(png_url())),
            ("in a subdirectory", "img/logo.svg", Some(svg_url)),
            ("up a directory", "../shared/up.png", Some(png_url())),
            ("percent escapes", "my%20pic.png", Some(png_url())),
            (
                "query and fragment ignored",
                "pic.png?raw=1#top",
                Some(png_url()),
            ),
            ("absolute", "/shared/up.png", Some(png_url())),
            ("missing", "gone.png", None),
            ("not an image", "readme.txt", None),
            ("too large", "huge.png", None),
            ("remote", "https://example.com/a.png", None),
            (
                "protocol relative is not a scheme but is not local",
                "//example.com/a.png",
                None,
            ),
            ("file scheme", "file:///notes/pic.png", None),
            (
                "data image kept",
                "data:image/png;base64,AAAA",
                Some("data:image/png;base64,AAAA".to_owned()),
            ),
            ("data that is not an image", "data:text/html,<b>x</b>", None),
            ("empty", "", None),
        ];
        for (name, url, want) in cases {
            assert_eq!(data_url(url, Some(&base), &files()), want, "{name}");
        }
    }

    #[test]
    fn a_relative_reference_needs_a_base_directory() {
        assert_eq!(data_url("pic.png", None, &files()), None);
        assert_eq!(data_url("/notes/pic.png", None, &files()), Some(png_url()));
    }

    #[test]
    fn no_files_means_no_images() {
        let base = FilePath::new("/notes").unwrap();
        assert_eq!(data_url("pic.png", Some(&base), &NoFiles), None);
    }

    #[test]
    fn percent_escapes_decode_and_malformed_ones_stay() {
        const CASES: &[(&str, &str)] = &[
            ("a%20b", "a b"),
            ("%C3%A9", "é"),
            ("100%", "100%"),
            ("%zz", "%zz"),
            ("%2", "%2"),
            ("plain", "plain"),
        ];
        for (text, want) in CASES {
            assert_eq!(percent_decode(text), *want, "{text}");
        }
    }
}
