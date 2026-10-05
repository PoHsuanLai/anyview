//! Tiny books built in memory with the zip crate, written into a scratch directory.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use anyview_core::FilePath;
use std::io::Write;
use zip::write::SimpleFileOptions;

/// A 1x1 PNG.
pub const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

pub fn write_zip(dir: &std::path::Path, name: &str, entries: &[(&str, &[u8])]) -> FilePath {
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (entry, bytes) in entries {
        zip.start_file(*entry, SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    FilePath::new(path).unwrap()
}

const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

const OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title>The Test Book</dc:title><dc:creator>Ann Author</dc:creator><dc:creator>Bo Writer</dc:creator>
<dc:publisher>Small Press</dc:publisher><dc:language>en</dc:language>
<meta name="cover" content="cover-img"/>
</metadata>
<manifest>
<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="c2" href="text/two.xhtml" media-type="application/xhtml+xml"/>
<item id="c1" href="text/one.xhtml" media-type="application/xhtml+xml"/>
<item id="css" href="style.css" media-type="text/css"/>
<item id="cover-img" href="img/cover.png" media-type="image/png"/>
<item id="pic" href="img/pic.png" media-type="image/png"/>
</manifest>
<spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>"#;

const NAV: &str = r##"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
<nav epub:type="toc"><ol>
<li><a href="text/one.xhtml">One</a><ol><li><a href="text/one.xhtml#s">One, part</a></li></ol></li>
<li><a href="text/two.xhtml">Two</a></li>
</ol></nav></body></html>"##;

const ONE: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>x</title>
<link rel="stylesheet" href="../style.css"/><script>alert(1)</script></head>
<body><h1>Chapter One</h1><p>Hello <img src="../img/pic.png" alt="pic"/></p>
<img src="https://example.com/remote.png" alt="remote"/></body></html>"#;

const TWO: &str = "<html><body><p>Second&nbsp;chapter</p></body></html>";

pub fn epub(dir: &std::path::Path) -> FilePath {
    write_zip(
        dir,
        "book.epub",
        &[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", CONTAINER.as_bytes()),
            ("OEBPS/content.opf", OPF.as_bytes()),
            ("OEBPS/nav.xhtml", NAV.as_bytes()),
            ("OEBPS/text/one.xhtml", ONE.as_bytes()),
            ("OEBPS/text/two.xhtml", TWO.as_bytes()),
            (
                "OEBPS/style.css",
                b"h1 { color: blue } p { background: url(img/pic.png) }",
            ),
            ("OEBPS/img/cover.png", PNG),
            ("OEBPS/img/pic.png", PNG),
        ],
    )
}

/// Pages named so that natural order differs from the zip's own and from plain text order.
pub fn comic(dir: &std::path::Path) -> FilePath {
    write_zip(
        dir,
        "comic.cbz",
        &[
            ("pages/10.png", PNG),
            ("pages/2.png", PNG),
            ("__MACOSX/pages/1.png", b"junk"),
            ("pages/notes.txt", b"not a page"),
            ("pages/1.png", PNG),
        ],
    )
}
