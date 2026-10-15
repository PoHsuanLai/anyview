//! The second step for an OLE compound file (the old binary Office formats): what it is, from the
//! names in its directory.

use super::Sniffed;
use crate::kind::{Family, FormatDetail, Mime, OfficeFormat};
use std::collections::HashSet;
use std::io;

/// The signature every compound file starts with.
const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The header's length; a sector follows it at the next sector boundary.
const HEADER_LEN: usize = 512;

/// Where the header keeps the sector size, the first directory sector and the FAT sector list.
const SHIFT_AT: usize = 0x1E;
const DIRECTORY_AT: usize = 0x30;
const FAT_LIST_AT: usize = 0x4C;
const FAT_LIST_LEN: usize = 109;

/// The bytes of a directory entry, and where its name lies in it.
const ENTRY_LEN: usize = 128;
const NAME_BYTES: usize = 64;
const NAME_LEN_AT: usize = 64;
const KIND_AT: usize = 66;

/// A sector id at or past this ends a chain or is a special sector.
const FIRST_SPECIAL: u32 = 0xFFFF_FFFA;

/// The most bytes of directory read, whatever sector size the header states: 1 MiB holds 8192
/// entries, far more than a document has.
const MAX_DIRECTORY_BYTES: u64 = 1024 * 1024;

/// The names in a compound file's directory: its streams and storages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OleStreams {
    names: Vec<String>,
}

/// Whether `head` starts like a compound file.
#[must_use]
pub fn is_ole(head: &super::FileHead) -> bool {
    head.bytes().starts_with(&SIGNATURE)
}

impl OleStreams {
    /// The directory names `names`.
    pub fn new<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        OleStreams {
            names: names.into_iter().map(Into::into).collect(),
        }
    }

    /// The directory of the compound file `read` gives access to: `read(offset, len)` returns up
    /// to `len` bytes at `offset`. `None` when it is not a compound file or its directory cannot
    /// be followed.
    pub fn read<R>(mut read: R) -> Option<Self>
    where
        R: FnMut(u64, u64) -> io::Result<Vec<u8>>,
    {
        let header = read(0, HEADER_LEN as u64).ok()?; // a small constant
        if header.len() < HEADER_LEN || !header.starts_with(&SIGNATURE) {
            return None;
        }
        let shift = u16::from_le_bytes(header.get(SHIFT_AT..SHIFT_AT + 2)?.try_into().ok()?);
        if !(7..=16).contains(&shift) {
            return None;
        }
        let size = 1_u64 << shift;
        let at = |sector: u32| (u64::from(sector) + 1) * size;
        let word = |bytes: &[u8], offset: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                bytes.get(offset..offset + 4)?.try_into().ok()?,
            ))
        };
        // The next sector of a chain, from the FAT sector the header lists for it.
        let next = |read: &mut R, sector: u32| -> Option<u32> {
            let per_fat = size / 4;
            let which = usize::try_from(u64::from(sector) / per_fat).ok()?;
            if which >= FAT_LIST_LEN {
                return None;
            }
            let fat = word(&header, FAT_LIST_AT + 4 * which)?;
            let entry = read(at(fat) + u64::from(sector) % per_fat * 4, 4).ok()?;
            word(&entry, 0)
        };

        let mut names = Vec::new();
        let mut sector = word(&header, DIRECTORY_AT)?;
        let mut left = MAX_DIRECTORY_BYTES;
        let mut visited = HashSet::new();
        // A chain that loops back on itself ends at the first sector seen twice.
        while sector < FIRST_SPECIAL && size <= left && visited.insert(sector) {
            left -= size;
            let bytes = read(at(sector), size).ok()?;
            for entry in bytes.as_chunks::<ENTRY_LEN>().0 {
                if let Some(name) = entry_name(entry) {
                    names.push(name);
                }
            }
            // A chain the header's FAT list cannot follow (past 109 FAT sectors) ends here, with the
            // names read so far.
            match next(&mut read, sector) {
                Some(following) => sector = following,
                None => break,
            }
        }
        Some(OleStreams { names })
    }

    fn has(&self, name: &str) -> bool {
        self.names.iter().any(|entry| entry == name)
    }
}

/// The name of a used directory entry.
fn entry_name(entry: &[u8]) -> Option<String> {
    if *entry.get(KIND_AT)? == 0 {
        return None;
    }
    let len = usize::from(u16::from_le_bytes(
        entry.get(NAME_LEN_AT..NAME_LEN_AT + 2)?.try_into().ok()?,
    ))
    .min(NAME_BYTES);
    let units: Vec<u16> = entry
        .get(..len)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// What a compound file is: Excel with a `Workbook` (or, from Excel 5, `Book`) stream, Word with a
/// `WordDocument` stream, PowerPoint with a `PowerPoint Document` stream. `None` for any other
/// compound file, such as an installer.
#[must_use]
pub fn sniff_ole(streams: &OleStreams) -> Option<Sniffed> {
    let format = if streams.has("Workbook") || streams.has("Book") {
        OfficeFormat::Xls
    } else if streams.has("WordDocument") {
        OfficeFormat::Doc
    } else if streams.has("PowerPoint Document") {
        OfficeFormat::Ppt
    } else {
        return None;
    };
    Some(Sniffed::new(
        format.kind(),
        Mime::known(format.mime()),
        FormatDetail::Office(format),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::FormatKind;
    use crate::sniff::FileHead;

    /// A compound file of 512-byte sectors: sector 0 is the FAT, sector 1 the directory holding
    /// `names` (four entries to a sector, so at most four).
    fn compound(names: &[&str]) -> Vec<u8> {
        let mut bytes = vec![0_u8; HEADER_LEN * 3];
        bytes[..8].copy_from_slice(&SIGNATURE);
        bytes[SHIFT_AT..SHIFT_AT + 2].copy_from_slice(&9_u16.to_le_bytes());
        bytes[DIRECTORY_AT..DIRECTORY_AT + 4].copy_from_slice(&1_u32.to_le_bytes());
        bytes[FAT_LIST_AT..FAT_LIST_AT + 4].copy_from_slice(&0_u32.to_le_bytes());
        // FAT: sector 0 is the FAT itself, sector 1 ends the directory chain.
        let fat = HEADER_LEN;
        bytes[fat..fat + 4].copy_from_slice(&0xFFFF_FFFD_u32.to_le_bytes());
        bytes[fat + 4..fat + 8].copy_from_slice(&0xFFFF_FFFE_u32.to_le_bytes());
        let directory = HEADER_LEN * 2;
        for (index, name) in names.iter().enumerate() {
            let entry = directory + index * ENTRY_LEN;
            let units: Vec<u16> = name.encode_utf16().collect();
            for (slot, unit) in units.iter().enumerate() {
                bytes[entry + slot * 2..entry + slot * 2 + 2].copy_from_slice(&unit.to_le_bytes());
            }
            let len = u16::try_from((units.len() + 1) * 2).unwrap();
            bytes[entry + NAME_LEN_AT..entry + NAME_LEN_AT + 2].copy_from_slice(&len.to_le_bytes());
            bytes[entry + KIND_AT] = 2;
        }
        bytes
    }

    fn streams_of(bytes: &[u8]) -> Option<OleStreams> {
        OleStreams::read(|offset, len| {
            let start = usize::try_from(offset).unwrap().min(bytes.len());
            let end = (start + usize::try_from(len).unwrap()).min(bytes.len());
            Ok(bytes[start..end].to_vec())
        })
    }

    #[test]
    fn a_directory_chain_that_loops_with_huge_sectors_is_read_once() {
        // Sectors of 64 KiB, the directory is sector 0, whose FAT entry points at itself.
        let mut bytes = vec![0_u8; 3 << 16];
        bytes[..8].copy_from_slice(&SIGNATURE);
        bytes[SHIFT_AT..SHIFT_AT + 2].copy_from_slice(&16_u16.to_le_bytes());
        let mut asked = 0_u64;
        let read = OleStreams::read(|offset, len| {
            asked += len;
            let start = usize::try_from(offset).unwrap().min(bytes.len());
            let end = (start + usize::try_from(len).unwrap()).min(bytes.len());
            Ok(bytes[start..end].to_vec())
        });
        assert_eq!(read, Some(OleStreams { names: Vec::new() }));
        assert!(
            asked <= HEADER_LEN as u64 + (64 << 10) + 4,
            "{asked} bytes asked"
        );
    }

    #[test]
    fn a_compound_file_is_named_by_its_directory() {
        // name, directory, kind and format
        type Case = (
            &'static str,
            &'static [&'static str],
            Option<(FormatKind, OfficeFormat)>,
        );
        const CASES: &[Case] = &[
            (
                "excel",
                &["Root Entry", "Workbook"],
                Some((FormatKind::Table, OfficeFormat::Xls)),
            ),
            (
                "excel 5",
                &["Root Entry", "Book"],
                Some((FormatKind::Table, OfficeFormat::Xls)),
            ),
            (
                "word",
                &["Root Entry", "1Table", "WordDocument"],
                Some((FormatKind::Office, OfficeFormat::Doc)),
            ),
            (
                "powerpoint",
                &["Root Entry", "PowerPoint Document"],
                Some((FormatKind::Office, OfficeFormat::Ppt)),
            ),
            ("an installer", &["Root Entry", "Property"], None),
        ];
        for (name, directory, want) in CASES {
            let streams = streams_of(&compound(directory)).unwrap();
            let got = sniff_ole(&streams).map(|sniffed| {
                let FormatDetail::Office(format) = sniffed.detail() else {
                    panic!("{name}: not an office detail");
                };
                (sniffed.kind(), *format)
            });
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn bytes_that_are_not_a_compound_file_have_no_directory() {
        assert!(streams_of(b"PK\x03\x04 not compound").is_none());
        assert!(!is_ole(&FileHead::new(b"PK\x03\x04")));
        assert!(is_ole(&FileHead::new(&compound(&[]))));
    }
}
