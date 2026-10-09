//! 7z files whose headers state sizes they do not have: each is refused without the 7z reader
//! allocating from the claim (an allocation that fails aborts the process, so a test that
//! returns has proved the claim never reached it).

use crate::support;

use anyview_archive::{ArchiveError, EntryLimit, ExtractLimits, extract, list};
use anyview_core::{ArchiveFormat, ByteLen};
use support::{ENTRIES, sevenz_of, write};

const SIGNATURE: [u8; 6] = [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C];

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// A 7z number of eight bytes: a first byte of ones, then the value.
fn number(value: u64) -> Vec<u8> {
    [&[0xFF][..], &value.to_le_bytes()].concat()
}

/// A 7z of `header` after the start header, which states `offset` and `size` for it.
fn with_start_header(offset: u64, size: u64, header: &[u8]) -> Vec<u8> {
    let mut tail = Vec::new();
    tail.extend_from_slice(&offset.to_le_bytes());
    tail.extend_from_slice(&size.to_le_bytes());
    tail.extend_from_slice(&crc32(header).to_le_bytes());
    let mut file = SIGNATURE.to_vec();
    file.extend_from_slice(&[0, 4]);
    file.extend_from_slice(&crc32(&tail).to_le_bytes());
    file.extend_from_slice(&tail);
    file.extend_from_slice(header);
    file
}

fn listed(bytes: &[u8]) -> Result<anyview_archive::Listing, ArchiveError> {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "hostile.7z", bytes);
    list(
        &path,
        ArchiveFormat::SevenZip,
        EntryLimit(100),
        ByteLen(64 * 1024 * 1024),
    )
}

#[test]
fn a_header_that_claims_more_than_the_file_holds_is_refused_before_anything_is_allocated() {
    // name, offset, size, header bytes that follow the start header
    let cases: Vec<(&str, u64, u64, Vec<u8>)> = vec![
        ("a terabyte", 0, 1 << 40, Vec::new()),
        ("a petabyte", 0, 1 << 50, Vec::new()),
        ("the largest size", 0, u64::MAX, Vec::new()),
        ("an offset past the end", u64::MAX - 8, 16, Vec::new()),
        ("a size past the end", 0, 100, vec![1, 0]),
    ];
    for (name, offset, size, header) in cases {
        let bytes = with_start_header(offset, size, &header);
        assert!(listed(&bytes).is_err(), "{name}");
    }
}

#[test]
fn a_header_that_names_billions_of_entries_is_refused() {
    // The header, a files-info block of 2^40 entries with no properties.
    let header = [&[0x01, 0x05][..], &number(1 << 40), &[0x00, 0x00]].concat();
    let bytes = with_start_header(0, header.len() as u64, &header);
    assert!(listed(&bytes).is_err());
}

#[test]
fn an_encoded_header_that_unpacks_to_a_terabyte_is_refused() {
    // An encoded header: one packed stream, one folder of one copy coder, an unpacked size of 2^40.
    let header = [
        &[0x17, 0x06, 0x00, 0x01, 0x09, 0x01, 0x00][..],
        &[0x07, 0x0B, 0x01, 0x00, 0x01, 0x01, 0x00, 0x0C],
        &number(1 << 40),
        &[0x00, 0x00],
    ]
    .concat();
    let bytes = with_start_header(0, header.len() as u64, &header);
    assert!(matches!(
        listed(&bytes),
        Err(ArchiveError::OverBudget { .. })
    ));
}

#[test]
fn an_lzma_coder_that_asks_for_a_four_gigabyte_dictionary_is_refused() {
    // A folder whose coder is LZMA with a dictionary of 4 GiB - 1, inside a real-looking header.
    let props = [&[0x5D][..], &u32::MAX.to_le_bytes()].concat();
    let header = [
        &[
            0x01, 0x04, 0x07, 0x0B, 0x01, 0x00, 0x01, 0x23, 0x03, 0x01, 0x01, 0x05,
        ][..],
        &props,
        &[0x0C, 0x01, 0x00, 0x00, 0x00],
    ]
    .concat();
    let bytes = with_start_header(0, header.len() as u64, &header);
    assert!(matches!(
        listed(&bytes),
        Err(ArchiveError::OverBudget { .. })
    ));
}

#[test]
fn a_real_7z_still_lists_and_extracts() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = sevenz_of(dir.path(), ENTRIES);
    let listing = listed(&bytes).unwrap();
    assert_eq!(listing.entries.len(), ENTRIES.len());
    let path = write(dir.path(), "real.7z", &bytes);
    let limits = ExtractLimits {
        entry: ByteLen(1024),
        scanned: ByteLen(1024),
    };
    let got = extract(&path, ArchiveFormat::SevenZip, "data.bin", limits).unwrap();
    assert_eq!(got, b"0123456789");
}
