//! Telling text from binary by its first bytes.

use super::FileHead;
use crate::kind::TextEncoding;

/// The encoding of a text head, or `None` for binary.
///
/// A UTF-16 byte-order mark makes a head text whatever follows it. Otherwise a head is text when
/// it holds no NUL and is valid UTF-8; a full head may end in the first bytes of a character the
/// 4 KiB cut in half, which does not count against it.
pub(super) fn encoding(head: &FileHead) -> Option<TextEncoding> {
    let bytes = head.bytes();
    match bytes {
        [0xFF, 0xFE, ..] => Some(TextEncoding::Utf16Le),
        [0xFE, 0xFF, ..] => Some(TextEncoding::Utf16Be),
        _ if bytes.contains(&0) => None,
        _ => valid_utf8(bytes, head).then_some(TextEncoding::Utf8),
    }
}

/// Whether a head that is not UTF-8 and has no byte-order mark still reads as text in some legacy
/// single- or double-byte encoding (Windows-1252, Shift_JIS, GBK, KOI8-R): no NUL, and at most one
/// byte in twenty a control character other than tab, line feed, form feed, carriage return or
/// escape. The bytes at 0x80 and above are all taken as printable, since which encoding they are in
/// is not decided here. A head that is valid UTF-8 is [`encoding`]'s, not this function's.
pub(super) fn is_legacy_text(head: &FileHead) -> bool {
    let bytes = head.bytes();
    if bytes.is_empty() || bytes.contains(&0) {
        return false;
    }
    let control = bytes
        .iter()
        .filter(|byte| matches!(byte, 0x01..=0x08 | 0x0B | 0x0E..=0x1A | 0x1C..=0x1F | 0x7F))
        .count();
    control * 20 <= bytes.len()
}

fn valid_utf8(bytes: &[u8], head: &FileHead) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none() && head.is_full(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_are_classified_as_text_or_binary() {
        const CASES: &[(&str, &[u8], Option<TextEncoding>)] = &[
            ("ascii", b"fn main() {}\n", Some(TextEncoding::Utf8)),
            ("empty", b"", Some(TextEncoding::Utf8)),
            (
                "utf-8 with a bom",
                b"\xEF\xBB\xBFhello",
                Some(TextEncoding::Utf8),
            ),
            (
                "multi-byte text",
                "héllo wörld ✓".as_bytes(),
                Some(TextEncoding::Utf8),
            ),
            (
                "utf-16 le bom",
                b"\xFF\xFEh\0i\0",
                Some(TextEncoding::Utf16Le),
            ),
            (
                "utf-16 be bom",
                b"\xFE\xFF\0h\0i",
                Some(TextEncoding::Utf16Be),
            ),
            ("a nul", b"abc\0def", None),
            ("invalid utf-8", b"abc\xFF\xFEdef", None),
            ("a character cut off in a short head", b"abc\xE2\x9C", None),
            ("png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", None),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(encoding(&FileHead::new(bytes)), *want, "{name}");
        }
    }

    #[test]
    fn a_head_that_is_not_utf8_may_be_legacy_text() {
        const CASES: &[(&str, &[u8], bool)] = &[
            ("windows-1252", b"caf\xE9 cr\xE8me\r\n", true),
            (
                "shift_jis",
                b"\x82\xB1\x82\xF1\x82\xC9\x82\xBF\x82\xCD\n",
                true,
            ),
            ("koi8-r", b"\xF0\xD2\xC9\xD7\xC5\xD4, \xCD\xC9\xD2\n", true),
            ("a nul", b"caf\xE9\0", false),
            ("empty", b"", false),
            (
                "mostly control bytes",
                b"\x01\x02\x03\x04\x05\x06\xE9\xE9\x07\x08",
                false,
            ),
            (
                "a stray control byte in a long line",
                b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x01",
                true,
            ),
            (
                "a binary header with high bytes",
                b"\x01\x02\x03\x04abc\xE9",
                false,
            ),
        ];
        for (name, bytes, want) in CASES {
            assert_eq!(is_legacy_text(&FileHead::new(bytes)), *want, "{name}");
        }
    }

    #[test]
    fn a_full_head_may_end_inside_a_character() {
        let mut bytes = vec![b'a'; 4094];
        bytes.extend_from_slice(&[0xE2, 0x9C]); // the first two bytes of a three-byte character
        assert_eq!(encoding(&FileHead::new(&bytes)), Some(TextEncoding::Utf8));
        bytes[100] = 0xFF; // a bad byte earlier is still invalid
        assert_eq!(encoding(&FileHead::new(&bytes)), None);
    }
}
