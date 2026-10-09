//! Archives built in memory and written to a scratch folder, one function per container.
// Each test crate uses some of these helpers, and none of them is a `#[test]` function.
#![allow(dead_code)]

use anyview_core::FilePath;
use std::io::Write;
use std::path::Path;

/// What every fixture holds: a folder and two files, in this order.
pub const ENTRIES: &[(&str, &str)] = &[
    ("docs/", ""),
    ("docs/readme.txt", "hello archive"),
    ("data.bin", "0123456789"),
];

/// `bytes` written as `name` in `dir`.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> FilePath {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    FilePath::new(path).unwrap()
}

/// A zip of [`ENTRIES`].
pub fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in entries {
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

/// A tar of `entries`, with a symbolic link called `link` to `data.bin` at the end.
pub fn tar_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, body) in entries {
        let mut header = tar::Header::new_gnu();
        if name.ends_with('/') {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            builder
                .append_data(&mut header, name, std::io::empty())
                .unwrap();
        } else {
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(&mut header, name, body.as_bytes())
                .unwrap();
        }
    }
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    builder.append_link(&mut link, "link", "data.bin").unwrap();
    builder.into_inner().unwrap()
}

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

pub fn bzip2(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

pub fn xz(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    lzma_rs::xz_compress(&mut std::io::BufReader::new(bytes), &mut out).unwrap();
    out
}

pub fn zstd(bytes: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(bytes, ruzstd::encoding::CompressionLevel::Fastest)
}

/// A 7z of `entries`, written through a file because the writer wants a path.
pub fn sevenz_of(dir: &Path, entries: &[(&str, &str)]) -> Vec<u8> {
    let path = dir.join("built.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    for (name, body) in entries {
        if name.ends_with('/') {
            let entry = sevenz_rust2::ArchiveEntry::new_directory(name.trim_end_matches('/'));
            writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
        } else {
            let entry = sevenz_rust2::ArchiveEntry::new_file(name);
            writer
                .push_archive_entry(entry, Some(body.as_bytes()))
                .unwrap();
        }
    }
    writer.finish().unwrap();
    std::fs::read(path).unwrap()
}
