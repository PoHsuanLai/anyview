//! Archive and compression formats whose entries the viewer lists.

use super::family::Family;
use ds_core::word::Word;

/// An archive or compressed-file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum ArchiveFormat {
    /// A zip archive that is none of the zip-based formats.
    Zip,
    /// A tar archive.
    Tar,
    /// A gzip stream, often a tarball.
    Gzip,
    /// A bzip2 stream.
    Bzip2,
    /// An xz stream.
    Xz,
    /// A Zstandard stream.
    Zstd,
    /// A 7z archive.
    SevenZip,
    /// A Java archive.
    Jar,
    /// An Android package.
    Apk,
}

impl Family for ArchiveFormat {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            ArchiveFormat::Zip => &["zip"],
            ArchiveFormat::Tar => &["tar"],
            ArchiveFormat::Gzip => &["gz", "tgz"],
            ArchiveFormat::Bzip2 => &["bz2", "tbz2"],
            ArchiveFormat::Xz => &["xz", "txz"],
            ArchiveFormat::Zstd => &["zst"],
            ArchiveFormat::SevenZip => &["7z"],
            ArchiveFormat::Jar => &["jar"],
            ArchiveFormat::Apk => &["apk"],
        }
    }

    fn mime(self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "application/zip",
            ArchiveFormat::Tar => "application/x-tar",
            ArchiveFormat::Gzip => "application/gzip",
            ArchiveFormat::Bzip2 => "application/x-bzip2",
            ArchiveFormat::Xz => "application/x-xz",
            ArchiveFormat::Zstd => "application/zstd",
            ArchiveFormat::SevenZip => "application/x-7z-compressed",
            ArchiveFormat::Jar => "application/java-archive",
            ArchiveFormat::Apk => "application/vnd.android.package-archive",
        }
    }
}
