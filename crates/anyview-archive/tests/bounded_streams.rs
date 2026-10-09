//! Compressed streams that unpack to far more than the cap stay inside a small memory bound.
//! This file holds one test: it measures the process's resident memory while a stream is
//! unpacked, so nothing else may run beside it.
//!
//! Its own binary, in a process of its own: it resets the kernel's high-water mark of the process's memory, so no other test may run beside it.

#![allow(clippy::unwrap_used)]

#[path = "archive/support/mod.rs"]
mod support;

use anyview_archive::{ExtractLimits, extract};
use anyview_core::{ArchiveFormat, ByteLen};
use support::{bzip2, gzip, write, xz, zstd};

/// Resident memory in KiB: the current size and the high-water mark since it was last reset.
fn resident() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let field = |name: &str| -> u64 {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap()
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// The most resident bytes `run` adds to what the process held when it started: the kernel's
/// high-water mark is reset first (writing 5 to `clear_refs`), so what ran before does not count.
fn peak_of(run: impl FnOnce()) -> u64 {
    std::fs::write("/proc/self/clear_refs", "5").unwrap();
    let (before, _) = resident();
    run();
    let (_, most) = resident();
    most.saturating_sub(before) * 1024
}

/// What a stream unpacks to: far more than the cap, and all zeros so it packs small.
const UNPACKED: usize = 24 * 1024 * 1024;

/// What the caller allows to be unpacked.
const CAP: u64 = 256 * 1024;

/// The most live bytes a capped unpack may add: the cap, its copies, and a decoder's window.
const ROOM: u64 = 6 * 1024 * 1024;

#[test]
fn a_stream_that_unpacks_to_far_more_than_the_cap_never_holds_more_than_a_window() {
    let dir = tempfile::tempdir().unwrap();
    let zeros = vec![0u8; UNPACKED];
    // name, packed stream, file name, format
    let cases: Vec<(&str, Vec<u8>, &str, ArchiveFormat)> = vec![
        ("gzip", gzip(&zeros), "bomb.gz", ArchiveFormat::Gzip),
        ("bzip2", bzip2(&zeros), "bomb.bz2", ArchiveFormat::Bzip2),
        ("xz", xz(&zeros), "bomb.xz", ArchiveFormat::Xz),
        ("zstd", zstd(&zeros), "bomb.zst", ArchiveFormat::Zstd),
    ];
    drop(zeros);
    for (name, packed, file, format) in cases {
        let path = write(dir.path(), file, &packed);
        let limits = ExtractLimits {
            entry: ByteLen(CAP),
            scanned: ByteLen(CAP),
        };
        let mut result = None;
        let added = peak_of(|| result = Some(extract(&path, format, "", limits)));
        let result = result.unwrap();
        assert!(
            matches!(result, Err(anyview_archive::ArchiveError::TooLarge { .. })),
            "{name}: {result:?}"
        );
        assert!(
            added <= ROOM,
            "{name}: {added} resident bytes at the peak for a cap of {CAP}"
        );
    }
}
