//! Images in a Markdown document. Quire's sealed frames load only `data:` URLs, so a local image
//! is read through an injected reader and written into the HTML as one.

use super::links::scheme;
use anyview_core::{FileHead, FileName, FilePath, FormatKind, SniffStep, sniff};
use std::collections::HashMap;
use std::io::Read;

/// The largest image, in bytes, that is inlined: a `data:` URL is a third larger than the file and
/// lives in the HTML string.
const MAX_INLINE: usize = 8 * 1024 * 1024;

/// The most that all the images of one document add to its HTML, counted in the `data:` URLs
/// written: a document may name the same large file again and again, and each mention is a copy.
const MAX_DOCUMENT_INLINE: usize = 32 * 1024 * 1024;

/// Reads the files a document refers to. The viewer's edge implements it over the disk and a test
/// over a map, so rendering stays pure.
pub trait LocalFiles {
    /// The bytes of the file at `path`, or `None` when it cannot be read, is not a regular file
    /// (a device, a pipe, a folder) or is longer than `most` bytes. A reader never holds more than
    /// `most` bytes of a file, whatever the file claims to be.
    fn read(&self, path: &FilePath, most: usize) -> Option<Vec<u8>>;
}

/// No files at all: every local image is shown as its alt text. A peek renders with this.
#[derive(Debug, Clone, Copy)]
pub struct NoFiles;

impl LocalFiles for NoFiles {
    fn read(&self, _: &FilePath, _: usize) -> Option<Vec<u8>> {
        None
    }
}

/// The files of the disk: a document's images are read from where it lies. The edge's reader.
#[derive(Debug, Clone, Copy)]
pub struct DiskFiles;

impl LocalFiles for DiskFiles {
    fn read(&self, path: &FilePath, most: usize) -> Option<Vec<u8>> {
        // Asked before opening, because opening a named pipe waits for a writer, and again after,
        // so the file that is read is the file that was looked at.
        if !std::fs::metadata(path.as_path()).ok()?.is_file() {
            return None;
        }
        let file = std::fs::File::open(path.as_path()).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(u64::try_from(most).ok()?.saturating_add(1))
            .read_to_end(&mut bytes)
            .ok()?;
        (bytes.len() <= most).then_some(bytes)
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

/// The images of one document: what has been read, and how much of the document's allowance the
/// `data:` URLs written so far have spent.
pub(super) struct Inliner<'a> {
    base: Option<&'a FilePath>,
    files: &'a dyn LocalFiles,
    /// The `data:` URL of each path asked for, `None` for a path that is not an image or could
    /// not be read: a file named twice is read once.
    urls: HashMap<FilePath, Option<String>>,
    spent: usize,
}

impl<'a> Inliner<'a> {
    pub(super) fn new(base: Option<&'a FilePath>, files: &'a dyn LocalFiles) -> Self {
        Inliner {
            base,
            files,
            urls: HashMap::new(),
            spent: 0,
        }
    }

    /// The `data:` URL an image reference becomes, or `None` when it must not be shown: a remote
    /// or otherwise non-local address, a file that cannot be read, is too large or is not an
    /// image, or an image that would take the document past its allowance. A reference that
    /// already is a `data:image/` URL is kept.
    pub(super) fn data_url(&mut self, url: &str) -> Option<String> {
        let url = url.trim();
        match scheme(url).as_deref() {
            Some("data") => return url.starts_with("data:image/").then(|| url.to_owned()),
            Some(_) => return None,
            None => {}
        }
        let path = target(url, self.base)?;
        let inlined = match self.urls.get(&path) {
            Some(known) => known.clone(),
            None => {
                let read = self.read_image(&path);
                self.urls.insert(path, read.clone());
                read
            }
        }?;
        let spent = self.spent.checked_add(inlined.len())?;
        if spent > MAX_DOCUMENT_INLINE {
            return None;
        }
        self.spent = spent;
        Some(inlined)
    }

    fn read_image(&self, path: &FilePath) -> Option<String> {
        let bytes = self.files.read(path, MAX_INLINE)?;
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
        fn read(&self, path: &FilePath, most: usize) -> Option<Vec<u8>> {
            let bytes = self.0.get(path.as_path().to_str()?)?;
            (bytes.len() <= most).then(|| bytes.clone())
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
            assert_eq!(
                Inliner::new(Some(&base), &files()).data_url(url),
                want,
                "{name}"
            );
        }
    }

    #[test]
    fn a_relative_reference_needs_a_base_directory() {
        assert_eq!(Inliner::new(None, &files()).data_url("pic.png"), None);
        assert_eq!(
            Inliner::new(None, &files()).data_url("/notes/pic.png"),
            Some(png_url())
        );
    }

    #[test]
    fn no_files_means_no_images() {
        let base = FilePath::new("/notes").unwrap();
        assert_eq!(
            Inliner::new(Some(&base), &NoFiles).data_url("pic.png"),
            None
        );
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

    /// A reader that counts the files it is asked for.
    struct Counting<'a>(&'a Files, std::cell::Cell<usize>);

    impl LocalFiles for Counting<'_> {
        fn read(&self, path: &FilePath, most: usize) -> Option<Vec<u8>> {
            self.1.set(self.1.get() + 1);
            self.0.read(path, most)
        }
    }

    #[test]
    fn a_file_named_again_is_read_once() {
        let files = files();
        let counting = Counting(&files, std::cell::Cell::new(0));
        let mut inliner = Inliner::new(None, &counting);
        for _ in 0..5 {
            assert_eq!(inliner.data_url("/notes/pic.png"), Some(png_url()));
        }
        assert_eq!(inliner.data_url("/notes/missing.png"), None);
        assert_eq!(inliner.data_url("/notes/missing.png"), None);
        assert_eq!(counting.1.get(), 2);
    }

    #[test]
    fn a_document_stops_inlining_when_its_images_have_spent_the_allowance() {
        let big = [PNG, &vec![0; MAX_INLINE - PNG.len()]].concat();
        let files = Files(HashMap::from([("/notes/big.png".to_owned(), big)]));
        let copy = Inliner::new(None, &files)
            .data_url("/notes/big.png")
            .unwrap()
            .len();
        let mut inliner = Inliner::new(None, &files);
        let shown = (0..2000)
            .filter(|_| inliner.data_url("/notes/big.png").is_some())
            .count();
        assert_eq!(shown, MAX_DOCUMENT_INLINE / copy);
    }

    #[test]
    fn the_disk_reads_regular_files_only_and_never_more_than_asked() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str| FilePath::new(dir.path().join(name)).unwrap();
        std::fs::write(file("pic.png").as_path(), PNG).unwrap();
        std::fs::create_dir(file("folder").as_path()).unwrap();
        let zero = FilePath::new("/dev/zero").unwrap();
        // name, path, most, bytes
        let mut cases: Vec<(&str, FilePath, usize, Option<Vec<u8>>)> = vec![
            ("a file", file("pic.png"), MAX_INLINE, Some(PNG.to_vec())),
            (
                "a file exactly at the limit",
                file("pic.png"),
                PNG.len(),
                Some(PNG.to_vec()),
            ),
            (
                "a file one past the limit",
                file("pic.png"),
                PNG.len() - 1,
                None,
            ),
            ("a folder", file("folder"), MAX_INLINE, None),
            ("a device that never ends", zero, MAX_INLINE, None),
            (
                "a path that is not there",
                file("gone.png"),
                MAX_INLINE,
                None,
            ),
        ];
        // rustix has no mkfifo on Apple platforms; Linux is where the pipe is tried.
        #[cfg(target_os = "linux")]
        {
            let pipe = file("pipe.png");
            rustix::fs::mkfifoat(
                rustix::fs::CWD,
                pipe.as_path(),
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap();
            cases.push(("a named pipe", pipe, MAX_INLINE, None));
        }
        for (name, path, most, want) in cases {
            assert_eq!(DiskFiles.read(&path, most), want, "{name}");
        }
    }
}
