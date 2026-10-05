//! What every text peek starts from: the first bytes of the file within the byte budget, decoded.

use crate::bytes::{ByteSource, FileBytes};
use crate::encoding::{Coverage, TextCodec, detect};
use crate::error::TextError;
use anyview_core::{ByteLen, FormatKind, PeekBudget, Sniffed, Source};

/// The lines (or rows) a peek shows. Every pane shows about this many, so the limit is the
/// pane's, not the budget's: the byte budget decides how much is read to find them.
pub const PEEK_LINES: usize = 40;

/// The start of a text file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Head {
    /// The decoded text. When the file is longer than the budget it ends at a line break, so no
    /// half line is shown.
    pub text: String,
    /// How the bytes were decoded.
    pub codec: TextCodec,
    /// Whether `text` is the whole file or only its start.
    pub coverage: Coverage,
}

/// Refuses a file that is not of the `expected` kind.
pub(crate) fn expect_kind(sniffed: &Sniffed, expected: FormatKind) -> Result<(), TextError> {
    if sniffed.kind() == expected {
        Ok(())
    } else {
        Err(TextError::WrongKind {
            kind: sniffed.kind(),
        })
    }
}

/// The first `budget.bytes` bytes of the file, decoded.
pub(crate) fn read_head(src: &Source, budget: &PeekBudget) -> Result<Head, TextError> {
    read_limited(src, budget.bytes)
}

/// The first `limit` bytes of the file, decoded.
pub(crate) fn read_limited(src: &Source, limit: ByteLen) -> Result<Head, TextError> {
    if limit.0 == 0 {
        return Err(TextError::NoBudget);
    }
    let file = FileBytes::open(src)?;
    let bytes = file.read(0..limit.0)?;
    let coverage = if file.byte_len().0 > bytes.len() as u64 {
        Coverage::Prefix
    } else {
        Coverage::Whole
    };
    Ok(decode_head(&bytes, coverage))
}

/// `bytes`, the start of a file, decoded and cut at its last line break when it is only a start.
pub(crate) fn decode_head(bytes: &[u8], coverage: Coverage) -> Head {
    let detected = detect(bytes, coverage);
    let body = bytes.get(usize::from(detected.mark)..).unwrap_or_default();
    let decoded = detected.codec.decode(body);
    let text = match coverage {
        Coverage::Whole => decoded.into_owned(),
        Coverage::Prefix => match decoded.rfind('\n') {
            Some(end) => decoded[..=end].to_owned(),
            None => decoded.into_owned(),
        },
    };
    Head {
        text,
        codec: detected.codec,
        coverage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::TextEncoding;

    #[test]
    fn a_prefix_is_cut_at_its_last_line_break_and_a_whole_file_is_kept() {
        // name, bytes, coverage, text
        const CASES: &[(&str, &[u8], Coverage, &str)] = &[
            ("whole", b"a\nb", Coverage::Whole, "a\nb"),
            ("prefix with a half line", b"a\nbc", Coverage::Prefix, "a\n"),
            (
                "prefix ending on a break",
                b"a\nb\n",
                Coverage::Prefix,
                "a\nb\n",
            ),
            ("prefix with no break", b"abc", Coverage::Prefix, "abc"),
            (
                "prefix with a cut character",
                b"a\nca\xC3",
                Coverage::Prefix,
                "a\n",
            ),
            (
                "byte-order mark skipped",
                b"\xEF\xBB\xBFa\n",
                Coverage::Whole,
                "a\n",
            ),
        ];
        for (name, bytes, coverage, text) in CASES {
            assert_eq!(decode_head(bytes, *coverage).text, *text, "{name}");
        }
    }

    #[test]
    fn the_encoding_is_reported_with_the_text() {
        let head = decode_head(b"\xFF\xFEa\0\n\0", Coverage::Whole);
        assert_eq!(head.codec, TextCodec::Unicode(TextEncoding::Utf16Le));
        assert_eq!(head.text, "a\n");
    }
}
