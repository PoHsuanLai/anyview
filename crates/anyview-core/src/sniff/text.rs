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
    fn a_full_head_may_end_inside_a_character() {
        let mut bytes = vec![b'a'; 4094];
        bytes.extend_from_slice(&[0xE2, 0x9C]); // the first two bytes of a three-byte character
        assert_eq!(encoding(&FileHead::new(&bytes)), Some(TextEncoding::Utf8));
        bytes[100] = 0xFF; // a bad byte earlier is still invalid
        assert_eq!(encoding(&FileHead::new(&bytes)), None);
    }
}
