//! WOFF 1: a font whose tables are each zlib-compressed, unpacked to the plain sfnt font it wraps.

use crate::error::FontError;

/// The most all the tables of one font may unpack to together. A directory may name tens of
/// thousands of tables, so the sum is bounded, not each table.
const UNPACKED_LIMIT: usize = 64 * 1024 * 1024;

const HEADER: usize = 44;
const ENTRY: usize = 20;

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

/// One entry of the WOFF table directory, before its data is unpacked.
struct Entry<'a> {
    tag: &'a [u8],
    checksum: u32,
    stored: &'a [u8],
    offset: usize,
    original: usize,
}

/// One unpacked table.
struct Table<'a> {
    tag: &'a [u8],
    checksum: u32,
    data: Vec<u8>,
}

fn entry<'a>(bytes: &'a [u8], at: usize) -> Result<Entry<'a>, FontError> {
    let short = || malformed("the WOFF table directory is cut short");
    let tag = bytes.get(at..at + 4).ok_or_else(short)?;
    let offset = be32(bytes, at + 4).ok_or_else(short)? as usize;
    let compressed = be32(bytes, at + 8).ok_or_else(short)? as usize;
    let original = be32(bytes, at + 12).ok_or_else(short)? as usize;
    let checksum = be32(bytes, at + 16).ok_or_else(short)?;
    let stored = bytes
        .get(offset..offset.checked_add(compressed).ok_or_else(short)?)
        .ok_or_else(|| malformed("a WOFF table lies outside the file"))?;
    if compressed > original {
        return Err(malformed("a WOFF table has an impossible size"));
    }
    Ok(Entry {
        tag,
        checksum,
        stored,
        offset,
        original,
    })
}

/// The directory of a WOFF file, refused when the tables would unpack to more than
/// [`UNPACKED_LIMIT`] together or when two of them share stored bytes: every table has its own.
fn directory(bytes: &[u8], count: usize) -> Result<Vec<Entry<'_>>, FontError> {
    let entries = (0..count)
        .map(|index| entry(bytes, HEADER + index * ENTRY))
        .collect::<Result<Vec<_>, _>>()?;
    let unpacked = entries
        .iter()
        .try_fold(0_usize, |sum, e| sum.checked_add(e.original));
    if unpacked.is_none_or(|sum| sum > UNPACKED_LIMIT) {
        return Err(malformed("the WOFF tables unpack to too much"));
    }
    let mut spans: Vec<(usize, usize)> = entries
        .iter()
        .filter(|e| !e.stored.is_empty())
        .map(|e| (e.offset, e.offset + e.stored.len()))
        .collect();
    spans.sort_unstable();
    if spans.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Err(malformed("WOFF tables share data"));
    }
    Ok(entries)
}

fn unpack<'a>(entry: &Entry<'a>) -> Result<Table<'a>, FontError> {
    let data = if entry.stored.len() == entry.original {
        entry.stored.to_vec()
    } else {
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(entry.stored, entry.original)
            .map_err(|_| malformed("a WOFF table does not unpack"))?
    };
    if data.len() != entry.original {
        return Err(malformed("a WOFF table unpacked to the wrong length"));
    }
    Ok(Table {
        tag: entry.tag,
        checksum: entry.checksum,
        data,
    })
}

/// The sfnt font (TrueType or OpenType) that the WOFF file `bytes` wraps.
pub(crate) fn to_sfnt(bytes: &[u8]) -> Result<Vec<u8>, FontError> {
    if bytes.get(..4) != Some(b"wOFF") {
        return Err(malformed("not a WOFF file"));
    }
    let flavor = be32(bytes, 4).ok_or_else(|| malformed("the WOFF header is cut short"))?;
    let count =
        usize::from(be16(bytes, 12).ok_or_else(|| malformed("the WOFF header is cut short"))?);
    let tables = directory(bytes, count)?
        .iter()
        .map(unpack)
        .collect::<Result<Vec<_>, _>>()?;
    let entry_selector = count.checked_ilog2().unwrap_or(0);
    let search_range = 16 * (1_usize << entry_selector);
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&u16::try_from(count).unwrap_or(u16::MAX).to_be_bytes());
    out.extend_from_slice(
        &u16::try_from(search_range)
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    out.extend_from_slice(&u16::try_from(entry_selector).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(
        &u16::try_from(16 * count - search_range.min(16 * count))
            .unwrap_or(0)
            .to_be_bytes(),
    );
    let mut offset = 12 + 16 * count;
    let mut body = Vec::new();
    for table in &tables {
        out.extend_from_slice(table.tag);
        out.extend_from_slice(&table.checksum.to_be_bytes());
        out.extend_from_slice(&u32::try_from(offset).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(
            &u32::try_from(table.data.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        body.extend_from_slice(&table.data);
        let padding = (4 - table.data.len() % 4) % 4;
        body.resize(body.len() + padding, 0);
        offset += table.data.len() + padding;
    }
    out.extend_from_slice(&body);
    Ok(out)
}
