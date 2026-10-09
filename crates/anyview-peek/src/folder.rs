//! The peek of a folder: how many items it holds, how large they are, and what kinds they are.
//!
//! One level only, never recursive: a launcher row must answer at once, whatever the folder.
//! Entries whose names start with a dot are not counted, as a file manager does not list them.

use crate::error::PeekError;
use anyview_core::{
    ByteLen, Deadline, FactLabel, FactValue, Facts, FileHead, FileName, FilePath, FormatKind,
    Input, Peek, PeekBudget, SniffStep, Sniffed, ZipEntries, open_regular, sniff, sniff_zip,
};
use anyview_text::Tally;
use ds_core::word::Word;
use std::fs::{self, DirEntry};
use std::io::Read;
use std::path::Path;
use std::time::Instant;

/// The most entries a peek counts; a folder with more is reported as "at least" this many.
pub const FOLDER_ENTRIES: usize = 10_000;

/// The bytes sniffing reads from a file's start.
const HEAD_BYTES: u64 = 4096;

/// The most kinds the facts name before they stop.
const KINDS_NAMED: usize = 3;

/// How many files of one kind a folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindCount {
    /// The kind.
    pub kind: FormatKind,
    /// How many of the files that were sniffed are of it.
    pub count: u32,
}

/// What a peek of a folder holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSummary {
    /// How many entries the folder has, as far as the cap let the peek count.
    pub items: Tally,
    /// How many of them are folders.
    pub folders: u32,
    /// How many of them are files.
    pub files: u32,
    /// The size of the files directly inside, added up.
    pub size: ByteLen,
    /// The kinds of the files that were sniffed, most common first. A file is sniffed only while
    /// the budget's bytes allow another read of its first 4 KiB, so on a large folder this names
    /// the kinds of its first files by name.
    pub kinds: Vec<KindCount>,
}

/// The peek of the kind `Folder`.
#[derive(Debug, Clone, Copy)]
pub struct FolderPeek;

impl Peek for FolderPeek {
    const KIND: FormatKind = FormatKind::Folder;
    type Peeked = FolderSummary;
    type Error = PeekError;

    fn peek(
        src: &Input,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<FolderSummary, PeekError> {
        if sniffed.kind() != Self::KIND {
            return Err(PeekError::WrongKind {
                kind: sniffed.kind(),
            });
        }
        // Listing a folder reads the directory itself: only a path has one.
        let Some(path) = src.path().map(FilePath::as_path) else {
            return Err(PeekError::Folder {
                path: src.label(),
                kind: std::io::ErrorKind::InvalidInput,
            });
        };
        // The listing and the sniffing are the loops that grow with the folder; both stop at the
        // budget's time and report what they had counted.
        let deadline = Deadline::of(budget, Instant::now());
        let mut entries = visible(path, deadline)?;
        let capped = entries.len() > FOLDER_ENTRIES;
        entries.truncate(FOLDER_ENTRIES);
        entries.sort_by_key(DirEntry::file_name);
        let mut reads_left = budget.bytes.0 / HEAD_BYTES;
        let (mut folders, mut files, mut size) = (0u32, 0u32, 0u64);
        let mut counts: Vec<KindCount> = Vec::new();
        for entry in &entries {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                folders += 1;
                continue;
            }
            if deadline.passed(Instant::now()) {
                reads_left = 0;
            }
            files += 1;
            size = size.saturating_add(entry.metadata().map_or(0, |meta| meta.len()));
            if reads_left > 0 && file_type.is_file() {
                reads_left -= 1;
                if let Some(kind) = kind_of(&entry.path(), &entry.file_name()) {
                    tally(&mut counts, kind);
                }
            }
        }
        counts.sort_by_key(|found| (std::cmp::Reverse(found.count), rank(found.kind)));
        let total = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        let items = match capped {
            true => Tally::AtLeast(total),
            false => Tally::Exact(total),
        };
        Ok(FolderSummary {
            items,
            folders,
            files,
            size: ByteLen(size),
            kinds: counts,
        })
    }

    fn facts(peeked: &FolderSummary) -> Facts {
        Facts::empty()
            .with(FactLabel::Kind, FactValue::text("Folder"))
            .with(FactLabel::Entries, FactValue::text(entries_text(peeked)))
            .with(FactLabel::Size, FactValue::size(peeked.size))
    }
}

/// The folder's entries that a file manager would list.
fn visible(path: &Path, deadline: Deadline) -> Result<Vec<DirEntry>, PeekError> {
    let folder_error = |error: std::io::Error| PeekError::Folder {
        path: path.to_path_buf(),
        kind: error.kind(),
    };
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(folder_error)? {
        let entry = entry.map_err(folder_error)?;
        if !entry.file_name().to_string_lossy().starts_with('.') {
            entries.push(entry);
        }
        // One more than the cap is enough to know the folder has more.
        if entries.len() > FOLDER_ENTRIES || deadline.passed(Instant::now()) {
            break;
        }
    }
    Ok(entries)
}

/// The kind of the file at `path`, sniffed from its first 4 KiB. A zip is not opened: with no
/// entry names it is an archive.
fn kind_of(path: &Path, name: &std::ffi::OsStr) -> Option<FormatKind> {
    let name = FileName::new(name.to_str()?).ok()?;
    let mut head = Vec::new();
    open_regular(path)
        .ok()?
        .0
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .ok()?;
    let sniffed = match sniff(&FileHead::new(&head), &name) {
        SniffStep::Done(sniffed) => sniffed,
        SniffStep::LookInside(probe) => {
            sniff_zip(probe, &ZipEntries::new(Vec::<String>::new(), None))
        }
    };
    Some(sniffed.kind())
}

fn tally(counts: &mut Vec<KindCount>, kind: FormatKind) {
    match counts.iter_mut().find(|found| found.kind == kind) {
        Some(found) => found.count += 1,
        None => counts.push(KindCount { kind, count: 1 }),
    }
}

/// A kind's place in the order the registry lists kinds, to break ties between equal counts.
fn rank(kind: FormatKind) -> usize {
    FormatKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(usize::MAX)
}

/// `12 items: 8 raster, 3 plain text`.
fn entries_text(summary: &FolderSummary) -> String {
    let noun = match summary.items {
        Tally::Exact(1) => "item",
        Tally::Exact(_) | Tally::AtLeast(_) => "items",
    };
    let head = format!("{} {noun}", summary.items.text());
    let kinds: Vec<String> = summary
        .kinds
        .iter()
        .take(KINDS_NAMED)
        .map(|found| format!("{} {}", found.count, found.kind.label().to_lowercase()))
        .collect();
    match kinds.is_empty() {
        true => head,
        false => format!("{head}: {}", kinds.join(", ")),
    }
}
