//! The header of a 7z, read and checked before the 7z crate sees the file.
//!
//! The crate sizes its buffers from numbers the file states (the next header's size, the entry
//! count, a property's size, an encoded header's unpacked size, a dictionary) and allocates before
//! it reads, so a 32-byte file can ask for a terabyte. Nothing here allocates from a claim: each
//! number is held against the file's length, a count limit or a byte limit first. An encoded
//! (compressed) header is unpacked here, inside its limit, so what it says is checked too.

use lzma_rust2::{Lzma2Reader, LzmaReader};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// The signature header: signature, version, start-header checksum, and the next header's place.
const START_HEADER: usize = 32;
const SIGNATURE: [u8; 6] = [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C];

/// The most bytes a header may take, packed or unpacked.
const HEADER_BYTES: u64 = 32 * 1024 * 1024;

/// The most entries, folders and packed streams a header may name: the crate keeps one record
/// of about a hundred bytes for each.
const MOST_ITEMS: u64 = 500_000;

/// The largest dictionary a coder may ask for: the decoder allocates it up front.
const MOST_DICTIONARY: u64 = 256 * 1024 * 1024;

/// The most coders, and the most streams in one coder, a folder may have.
const MOST_CODERS: u64 = 32;

const ENCODED_HEADER: u8 = 0x17;
const HEADER: u8 = 0x01;
const ARCHIVE_PROPERTIES: u8 = 0x02;
const ADDITIONAL_STREAMS: u8 = 0x03;
const MAIN_STREAMS: u8 = 0x04;
const FILES_INFO: u8 = 0x05;
const PACK_INFO: u8 = 0x06;
const UNPACK_INFO: u8 = 0x07;
const SUB_STREAMS: u8 = 0x08;
const SIZE: u8 = 0x09;
const CRC: u8 = 0x0A;
const FOLDER: u8 = 0x0B;
const CODERS_UNPACK_SIZE: u8 = 0x0C;
const NUM_UNPACK_STREAM: u8 = 0x0D;
const END: u8 = 0x00;

const LZMA: &[u8] = &[0x03, 0x01, 0x01];
const LZMA2: &[u8] = &[0x21];
const COPY: &[u8] = &[0x00];

/// Why a header is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// It asks for more than a listing may spend.
    Large,
    /// It does not hold together.
    Broken(&'static str),
}

type Checked<T> = Result<T, Fault>;

/// Checks the header of the 7z in `file`, `len` bytes long, which a listing may read `budget`
/// bytes of. Anything that is not a 7z passes: the crate names what is wrong with it.
pub(crate) fn check(file: &mut File, len: u64, budget: u64) -> Checked<()> {
    let mut start = [0u8; START_HEADER];
    if file.seek(SeekFrom::Start(0)).is_err() || file.read_exact(&mut start).is_err() {
        return Ok(());
    }
    if start[..6] != SIGNATURE {
        return Ok(());
    }
    let field = |from: usize| {
        let mut word = [0u8; 8];
        word.copy_from_slice(&start[from..from + 8]);
        u64::from_le_bytes(word)
    };
    let (offset, size) = (field(12), field(20));
    if start[8..].iter().all(|byte| *byte == 0) {
        return Err(Fault::Broken("the start header is empty"));
    }
    if size == 0 {
        return Ok(());
    }
    let end = (START_HEADER as u64)
        .checked_add(offset)
        .and_then(|from| from.checked_add(size))
        .ok_or(Fault::Broken("the header lies past the end"))?;
    if end > len {
        return Err(Fault::Broken("the header lies past the end"));
    }
    if size > HEADER_BYTES || size > budget {
        return Err(Fault::Large);
    }
    let packed = read_at(file, START_HEADER as u64 + offset, size)?;
    match packed.first().copied() {
        Some(HEADER) => walk_header(&mut Cursor(&packed[1..])),
        Some(ENCODED_HEADER) => {
            let unpacked = unpack_header(file, len, &packed[1..], budget)?;
            match unpacked.split_first() {
                Some((&HEADER, rest)) => walk_header(&mut Cursor(rest)),
                _ => Err(Fault::Broken("the header is not a header")),
            }
        }
        _ => Err(Fault::Broken("the header is not a header")),
    }
}

/// `size` bytes at `at`; the caller has held `size` against the file's length and a limit.
fn read_at(file: &mut File, at: u64, size: u64) -> Checked<Vec<u8>> {
    file.seek(SeekFrom::Start(at))
        .map_err(|_| Fault::Broken("the header cannot be reached"))?;
    let mut bytes = Vec::new();
    file.take(size)
        .read_to_end(&mut bytes)
        .map_err(|_| Fault::Broken("the header cannot be read"))?;
    if bytes.len() as u64 != size {
        return Err(Fault::Broken("the header is cut short"));
    }
    Ok(bytes)
}

/// The header an encoded header holds: one folder of one coder (LZMA, LZMA2 or none), unpacked
/// within [`HEADER_BYTES`] and `budget`.
fn unpack_header(file: &mut File, len: u64, encoded: &[u8], budget: u64) -> Checked<Vec<u8>> {
    let streams = streams_info(&mut Cursor(encoded))?;
    let (Some(folder), Some(&packed)) = (streams.folders.first(), streams.pack_sizes.first())
    else {
        return Err(Fault::Broken("the encoded header names no stream"));
    };
    let [coder] = folder.coders.as_slice() else {
        return Err(Fault::Broken("the encoded header is packed in layers"));
    };
    let unpacked = *streams
        .unpack_sizes
        .first()
        .and_then(|sizes| sizes.last())
        .ok_or(Fault::Broken("the encoded header has no size"))?;
    if unpacked > HEADER_BYTES || unpacked > budget {
        return Err(Fault::Large);
    }
    let at = (START_HEADER as u64)
        .checked_add(streams.pack_pos)
        .filter(|at| at.checked_add(packed).is_some_and(|end| end <= len))
        .ok_or(Fault::Broken("the encoded header lies past the end"))?;
    if packed > HEADER_BYTES {
        return Err(Fault::Large);
    }
    let raw = read_at(file, at, packed)?;
    let mut out = Vec::new();
    let reader: Box<dyn Read + '_> = match coder.id.as_slice() {
        id if id == COPY => Box::new(raw.as_slice()),
        id if id == LZMA => {
            let (props, dictionary) = lzma_props(&coder.props)?;
            Box::new(
                LzmaReader::new_with_props(raw.as_slice(), unpacked, props, dictionary, None)
                    .map_err(|_| Fault::Broken("the encoded header does not unpack"))?,
            )
        }
        id if id == LZMA2 => {
            let dictionary = lzma2_dictionary(&coder.props)?;
            Box::new(Lzma2Reader::new(raw.as_slice(), dictionary, None))
        }
        _ => return Err(Fault::Broken("the encoded header uses another method")),
    };
    reader
        .take(unpacked)
        .read_to_end(&mut out)
        .map_err(|_| Fault::Broken("the encoded header does not unpack"))?;
    if out.len() as u64 != unpacked {
        return Err(Fault::Broken("the encoded header is cut short"));
    }
    Ok(out)
}

/// A cursor over header bytes whose every read is held against what is left.
struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn byte(&mut self) -> Checked<u8> {
        let (first, rest) = self
            .0
            .split_first()
            .ok_or(Fault::Broken("the header ends early"))?;
        self.0 = rest;
        Ok(*first)
    }

    fn take(&mut self, count: u64) -> Checked<&'a [u8]> {
        let count = usize::try_from(count).map_err(|_| Fault::Broken("the header ends early"))?;
        if count > self.0.len() {
            return Err(Fault::Broken("the header ends early"));
        }
        let (taken, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(taken)
    }

    /// A 7z number: the leading ones of the first byte say how many bytes follow.
    fn number(&mut self) -> Checked<u64> {
        let first = u64::from(self.byte()?);
        let mut mask = 0x80_u64;
        let mut value = 0_u64;
        for i in 0..8 {
            if first & mask == 0 {
                return Ok(value | ((first & (mask - 1)) << (8 * i)));
            }
            value |= u64::from(self.byte()?) << (8 * i);
            mask >>= 1;
        }
        Ok(value)
    }

    /// A number that counts things, held to `most`.
    fn count(&mut self, most: u64) -> Checked<usize> {
        let number = self.number()?;
        match usize::try_from(number) {
            Ok(count) if number <= most => Ok(count),
            _ => Err(Fault::Large),
        }
    }

    /// Which of `count` items carry a checksum, and the checksums skipped.
    fn digests(&mut self, count: usize) -> Checked<Vec<bool>> {
        let defined = match self.byte()? {
            0 => {
                let bits = self.take(count.div_ceil(8) as u64)?;
                (0..count)
                    .map(|i| bits[i / 8] & (0x80 >> (i % 8)) != 0)
                    .collect()
            }
            _ => vec![true; count],
        };
        self.take(4 * defined.iter().filter(|set| **set).count() as u64)?;
        Ok(defined)
    }
}

/// One coder of a folder: its method and its properties.
struct Coder {
    id: Vec<u8>,
    props: Vec<u8>,
}

/// One folder: the coders that unpack one block, and how many streams they put out.
struct Folder {
    coders: Vec<Coder>,
    outputs: usize,
}

/// What a streams-info block says that the checks need.
#[derive(Default)]
struct Streams {
    pack_pos: u64,
    pack_sizes: Vec<u64>,
    folders: Vec<Folder>,
    unpack_sizes: Vec<Vec<u64>>,
    folder_crcs: Vec<bool>,
}

fn streams_info(c: &mut Cursor<'_>) -> Checked<Streams> {
    let mut streams = Streams::default();
    loop {
        match c.byte()? {
            END => return Ok(streams),
            PACK_INFO => {
                streams.pack_pos = c.number()?;
                let count = c.count(MOST_ITEMS)?;
                loop {
                    match c.byte()? {
                        END => break,
                        SIZE => {
                            streams.pack_sizes = (0..count)
                                .map(|_| c.number())
                                .collect::<Checked<Vec<u64>>>()?;
                        }
                        CRC => {
                            c.digests(count)?;
                        }
                        _ => return Err(Fault::Broken("a pack info property is unknown")),
                    }
                }
            }
            UNPACK_INFO => unpack_info(c, &mut streams)?,
            SUB_STREAMS => sub_streams(c, &streams)?,
            _ => return Err(Fault::Broken("a streams info property is unknown")),
        }
    }
}

fn unpack_info(c: &mut Cursor<'_>, streams: &mut Streams) -> Checked<()> {
    if c.byte()? != FOLDER {
        return Err(Fault::Broken("the folders are missing"));
    }
    let count = c.count(MOST_ITEMS)?;
    if c.byte()? != 0 {
        return Err(Fault::Broken("the folders are stored elsewhere"));
    }
    streams.folders = (0..count)
        .map(|_| folder(c))
        .collect::<Checked<Vec<Folder>>>()?;
    if c.byte()? != CODERS_UNPACK_SIZE {
        return Err(Fault::Broken("the folder sizes are missing"));
    }
    streams.unpack_sizes = streams
        .folders
        .iter()
        .map(|folder| (0..folder.outputs).map(|_| c.number()).collect())
        .collect::<Checked<Vec<Vec<u64>>>>()?;
    loop {
        match c.byte()? {
            END => return Ok(()),
            CRC => streams.folder_crcs = c.digests(count)?,
            _ => return Err(Fault::Broken("an unpack info property is unknown")),
        }
    }
}

fn folder(c: &mut Cursor<'_>) -> Checked<Folder> {
    let count = c.count(MOST_CODERS)?;
    let (mut inputs, mut outputs) = (0_u64, 0_u64);
    let mut coders = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = c.byte()?;
        if flags & 0x80 != 0 {
            return Err(Fault::Broken("a coder has alternative methods"));
        }
        let id = c.take(u64::from(flags & 0x0F))?.to_vec();
        let (ins, outs) = match flags & 0x10 {
            0 => (1, 1),
            _ => (c.count(MOST_CODERS)? as u64, c.count(MOST_CODERS)? as u64),
        };
        let props = match flags & 0x20 {
            0 => Vec::new(),
            _ => {
                let size = c.number()?;
                c.take(size)?.to_vec()
            }
        };
        check_dictionary(&id, &props)?;
        inputs += ins;
        outputs += outs;
        coders.push(Coder { id, props });
    }
    let pairs = outputs
        .checked_sub(1)
        .ok_or(Fault::Broken("a folder has no output"))?;
    for _ in 0..pairs {
        c.number()?;
        c.number()?;
    }
    let packed = inputs
        .checked_sub(pairs)
        .filter(|packed| *packed >= 1)
        .ok_or(Fault::Broken("a folder has no input"))?;
    if packed > 1 {
        for _ in 0..packed {
            c.number()?;
        }
    }
    Ok(Folder {
        coders,
        outputs: usize::try_from(outputs).unwrap_or(usize::MAX),
    })
}

fn sub_streams(c: &mut Cursor<'_>, streams: &Streams) -> Checked<()> {
    let folders = streams.folders.len();
    let mut counts = vec![1_usize; folders];
    loop {
        match c.byte()? {
            END => return Ok(()),
            NUM_UNPACK_STREAM => {
                let mut total = 0_u64;
                for slot in &mut counts {
                    *slot = c.count(MOST_ITEMS)?;
                    total += *slot as u64;
                    if total > MOST_ITEMS {
                        return Err(Fault::Large);
                    }
                }
            }
            SIZE => {
                for count in &counts {
                    for _ in 1..*count {
                        c.number()?;
                    }
                }
            }
            CRC => {
                // A folder of one stream whose own checksum is defined states none again.
                let wanted: usize = counts
                    .iter()
                    .enumerate()
                    .filter(|(i, count)| {
                        !(**count == 1 && streams.folder_crcs.get(*i).copied().unwrap_or(false))
                    })
                    .map(|(_, count)| *count)
                    .sum();
                c.digests(wanted)?;
            }
            _ => return Err(Fault::Broken("a sub-streams property is unknown")),
        }
    }
}

fn walk_header(c: &mut Cursor<'_>) -> Checked<()> {
    let mut id = c.byte()?;
    if id == ARCHIVE_PROPERTIES {
        loop {
            if c.byte()? == END {
                break;
            }
            let size = c.number()?;
            c.take(size)?;
        }
        id = c.byte()?;
    }
    if id == ADDITIONAL_STREAMS {
        return Err(Fault::Broken("the header has additional streams"));
    }
    if id == MAIN_STREAMS {
        streams_info(c)?;
        id = c.byte()?;
    }
    if id == FILES_INFO {
        c.count(MOST_ITEMS)?;
        loop {
            if c.byte()? == END {
                break;
            }
            let size = c.number()?;
            c.take(size)?;
        }
        id = c.byte()?;
    }
    match id {
        END => Ok(()),
        _ => Err(Fault::Broken("the header does not end")),
    }
}

/// Refuses a coder that would allocate a dictionary past [`MOST_DICTIONARY`].
fn check_dictionary(id: &[u8], props: &[u8]) -> Checked<()> {
    let dictionary = match id {
        id if id == LZMA => lzma_props(props)?.1,
        id if id == LZMA2 => lzma2_dictionary(props)?,
        _ => return Ok(()),
    };
    match u64::from(dictionary) > MOST_DICTIONARY {
        true => Err(Fault::Large),
        false => Ok(()),
    }
}

/// An LZMA coder's properties byte and dictionary size.
fn lzma_props(props: &[u8]) -> Checked<(u8, u32)> {
    match props {
        [byte, a, b, c, d, ..] => Ok((*byte, u32::from_le_bytes([*a, *b, *c, *d]))),
        _ => Err(Fault::Broken("an LZMA coder has no properties")),
    }
}

/// An LZMA2 coder's dictionary size: one byte, two bits of mantissa and the rest exponent.
fn lzma2_dictionary(props: &[u8]) -> Checked<u32> {
    match props.first() {
        Some(&40) => Ok(u32::MAX),
        Some(&byte) if byte < 40 => Ok((2 | u32::from(byte & 1)) << (byte / 2 + 11)),
        _ => Err(Fault::Broken("an LZMA2 coder has no valid properties")),
    }
}
