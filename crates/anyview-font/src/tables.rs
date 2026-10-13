//! Reading a font by its table directory: the peek reads the directory, then only the tables it
//! draws from, and puts them in a plain font of their own for skrifa. A large face (a CJK family,
//! a colour one) keeps most of its size in tables the peek never opens, so none of that is read.

use crate::error::FontError;
use crate::woff::{Table, sfnt};
use anyview_core::{ByteLen, Input};

/// The tables the peek reads: the names, the glyph count and variation, the character map, the
/// advances and the outlines (TrueType's `glyf` with its `loca`, or CFF).
const WANTED: &[&[u8; 4]] = &[
    b"CFF ", b"CFF2", b"OS/2", b"cmap", b"fvar", b"glyf", b"head", b"hhea", b"hmtx", b"loca",
    b"maxp", b"name", b"post",
];

/// The size of an sfnt header, and of a collection's.
const HEADER: u64 = 12;
/// The size of one entry of the table directory.
const RECORD: u64 = 16;

/// The first face of a font file, cut down to the tables the peek reads.
pub(crate) struct Sparse {
    /// The plain font those tables make.
    pub(crate) sfnt: Vec<u8>,
    /// How many faces the file holds.
    pub(crate) faces: u32,
}

/// One entry of a table directory.
struct Record {
    tag: [u8; 4],
    checksum: u32,
    offset: u64,
    length: u64,
}

/// Reads ranges of the file, counting every byte against the budget.
struct Reader<'a> {
    src: &'a Input,
    allowed: ByteLen,
    file_len: u64,
    spent: u64,
}

impl Reader<'_> {
    /// `length` bytes at `at`; the file must hold them and the budget must cover them.
    fn take(&mut self, at: u64, length: u64) -> Result<Vec<u8>, FontError> {
        let end = at
            .checked_add(length)
            .filter(|end| *end <= self.file_len)
            .ok_or_else(|| malformed("a font table lies outside the file"))?;
        self.spent = self.spent.saturating_add(length);
        if self.spent > self.allowed.0 {
            return Err(self.over());
        }
        let bytes = self
            .src
            .bytes()
            .read_range(at..end)
            .map_err(|error| FontError::Read {
                path: self.src.label(),
                kind: error.kind(),
            })?;
        if u64::try_from(bytes.len()).ok() != Some(length) {
            return Err(malformed("the font ends before its tables do"));
        }
        Ok(bytes)
    }

    fn over(&self) -> FontError {
        FontError::OverBudget {
            len: ByteLen(self.spent),
            allowed: self.allowed,
        }
    }
}

fn malformed(reason: &str) -> FontError {
    FontError::Malformed {
        reason: reason.to_owned(),
    }
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    let raw = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([raw[0], raw[1]]))
}

/// The first face of the font file `src`, with the tables the peek reads. The bytes it reads are
/// the directory and those tables, and no more than `allowed`: a face whose directory and wanted
/// tables together exceed it is [`FontError::OverBudget`].
pub(crate) fn sparse(src: &Input, allowed: ByteLen) -> Result<Sparse, FontError> {
    let mut reader = Reader {
        src,
        allowed,
        file_len: src.stamp().len.0.max(src.bytes().len().0),
        spent: 0,
    };
    let cut = || malformed("the font's table directory is cut short");
    let head = reader.take(0, HEADER.min(reader.file_len))?;
    let (faces, at, offset_table) = if head.get(..4) == Some(b"ttcf") {
        let faces = be32(&head, 8).ok_or_else(cut)?;
        if faces == 0 {
            return Err(malformed("the font collection holds no face"));
        }
        let first = u64::from(be32(&reader.take(HEADER, 4)?, 0).ok_or_else(cut)?);
        (faces, first, reader.take(first, HEADER)?)
    } else {
        (1, 0, head)
    };
    let flavor = be32(&offset_table, 0).ok_or_else(cut)?;
    let count = u64::from(be16(&offset_table, 4).ok_or_else(cut)?);
    let directory = reader.take(at + HEADER, count * RECORD)?;
    let records: Vec<Record> = directory
        .chunks_exact(usize::try_from(RECORD).unwrap_or(16))
        .filter_map(|raw| {
            Some(Record {
                tag: [raw[0], raw[1], raw[2], raw[3]],
                checksum: be32(raw, 4)?,
                offset: u64::from(be32(raw, 8)?),
                length: u64::from(be32(raw, 12)?),
            })
        })
        .filter(|record| WANTED.iter().any(|tag| **tag == record.tag))
        .collect();
    // The whole cost is known before a table is read.
    let needed = records.iter().fold(reader.spent, |sum, record| {
        sum.saturating_add(record.length)
    });
    if needed > allowed.0 {
        return Err(FontError::OverBudget {
            len: ByteLen(needed),
            allowed,
        });
    }
    let mut data = Vec::with_capacity(records.len());
    for record in &records {
        data.push(reader.take(record.offset, record.length)?);
    }
    let tables: Vec<Table<'_>> = records
        .iter()
        .zip(data)
        .map(|(record, data)| Table {
            tag: &record.tag,
            checksum: record.checksum,
            data,
        })
        .collect();
    Ok(Sparse {
        sfnt: sfnt(flavor, &tables),
        faces,
    })
}
