//! How each archive format is stored: the one place that matches on `ArchiveFormat`.

use anyview_core::ArchiveFormat;

/// How a compressed stream is coded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Codec {
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

/// The layout of an archive file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Container {
    /// A central directory at the end, entries compressed one by one (zip, jar, apk).
    Zip,
    /// A header per entry, none of it compressed: seeking skips an entry's bytes.
    Tar,
    /// One compressed stream: a tar when what it unpacks to is one, else a single file.
    Compressed(Codec),
    /// A header at the end naming folders of entries (7z).
    SevenZip,
}

/// The layout of `format`.
pub(crate) fn container(format: ArchiveFormat) -> Container {
    match format {
        ArchiveFormat::Zip | ArchiveFormat::Jar | ArchiveFormat::Apk => Container::Zip,
        ArchiveFormat::Tar => Container::Tar,
        ArchiveFormat::Gzip => Container::Compressed(Codec::Gzip),
        ArchiveFormat::Bzip2 => Container::Compressed(Codec::Bzip2),
        ArchiveFormat::Xz => Container::Compressed(Codec::Xz),
        ArchiveFormat::Zstd => Container::Compressed(Codec::Zstd),
        ArchiveFormat::SevenZip => Container::SevenZip,
    }
}

/// What a person calls the format: `ZIP`, `7z`, `Tar (gzip)`.
pub(crate) fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP",
        ArchiveFormat::Tar => "Tar",
        ArchiveFormat::Gzip => "gzip",
        ArchiveFormat::Bzip2 => "bzip2",
        ArchiveFormat::Xz => "xz",
        ArchiveFormat::Zstd => "Zstandard",
        ArchiveFormat::SevenZip => "7z",
        ArchiveFormat::Jar => "Java",
        ArchiveFormat::Apk => "Android",
    }
}

/// What a listing is of, in words: `ZIP archive`, `Tar (gzip)`, `gzip file`.
pub(crate) fn kind_name(format: ArchiveFormat, holds: crate::entry::Holds) -> String {
    use crate::entry::Holds;
    match (container(format), holds) {
        (Container::Compressed(_), Holds::Entries) => format!("Tar ({})", format_name(format)),
        (Container::Compressed(_), Holds::OneFile) => format!("{} file", format_name(format)),
        (
            Container::Zip | Container::Tar | Container::SevenZip,
            Holds::Entries | Holds::OneFile,
        ) => {
            format!("{} archive", format_name(format))
        }
    }
}
