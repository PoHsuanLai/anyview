//! What every text peek starts from: the first bytes of the file within the byte budget, decoded.

use crate::bytes::read_range;
use crate::encoding::{Coverage, TextCodec, detect};
use crate::error::TextError;
use anyview_core::work::Interrupt;
use anyview_core::{ByteLen, FormatKind, Input, PeekBudget, Sniffed};

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

/// How much of the head is read between two asks of the interrupt.
const HEAD_CHUNK: u64 = 1024 * 1024;

/// The first `budget.bytes` bytes of the file, decoded; fewer, as a start of the file, when
/// `interrupt` says to end before they are read.
pub(crate) fn read_head(
    src: &Input,
    budget: &PeekBudget,
    interrupt: &dyn Interrupt,
) -> Result<Head, TextError> {
    read_limited(src, budget.bytes, interrupt)
}

/// The first `limit` bytes of the file, decoded; fewer when `interrupt` says to end, but always
/// the first chunk, so a peek that is late still has a start to show.
pub(crate) fn read_limited(
    src: &Input,
    limit: ByteLen,
    interrupt: &dyn Interrupt,
) -> Result<Head, TextError> {
    if limit.0 == 0 {
        return Err(TextError::NoBudget);
    }
    let mut bytes = Vec::new();
    let mut at = 0_u64;
    while at < limit.0 {
        if at > 0 && interrupt.interrupted() {
            break;
        }
        let end = at.saturating_add(HEAD_CHUNK).min(limit.0);
        let chunk = read_range(src.bytes(), at..end)?;
        let short = (chunk.len() as u64) < end - at; // a usize fits a u64
        at += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);
        if short {
            break;
        }
    }
    let coverage = if src.bytes().len().0 > bytes.len() as u64 {
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
    let mut text = decoded.into_owned();
    if coverage == Coverage::Prefix
        && let Some(end) = text.rfind('\n')
    {
        text.truncate(end + 1);
    }
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
