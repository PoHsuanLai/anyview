//! Which encoding a text file's bytes are in, and turning them into text.

use anyview_core::TextEncoding;
use std::borrow::Cow;

/// How many bytes at the start of a file decide its encoding. Text past them that does not fit the
/// choice decodes with replacement characters instead of changing it.
pub const DETECT_BYTES: usize = 1 << 20;

/// An encoding the viewer decodes: what a byte-order mark or valid UTF-8 identifies, or the
/// single-byte fallback every other file gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextCodec {
    /// UTF-8, UTF-16 little endian or UTF-16 big endian.
    Unicode(TextEncoding),
    /// Windows-1252, the web's default for bytes that are not UTF-8. Every byte maps to a
    /// character, so decoding never fails.
    Windows1252,
}

/// What the start of a file says about how to read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Detected {
    /// The encoding.
    pub codec: TextCodec,
    /// The bytes of byte-order mark to skip: 0, 2 or 3.
    pub mark: u8,
}

impl TextCodec {
    /// `UTF-8`, `UTF-16 LE`, `UTF-16 BE` or `Windows-1252`, as a pane lists it.
    pub fn label(self) -> &'static str {
        match self {
            TextCodec::Unicode(TextEncoding::Utf8) => "UTF-8",
            TextCodec::Unicode(TextEncoding::Utf16Le) => "UTF-16 LE",
            TextCodec::Unicode(TextEncoding::Utf16Be) => "UTF-16 BE",
            TextCodec::Windows1252 => "Windows-1252",
        }
    }

    /// Bytes per unit the line breaks are written in: 2 for UTF-16, otherwise 1.
    pub(crate) fn unit(self) -> usize {
        match self {
            TextCodec::Unicode(TextEncoding::Utf16Le | TextEncoding::Utf16Be) => 2,
            TextCodec::Unicode(TextEncoding::Utf8) | TextCodec::Windows1252 => 1,
        }
    }

    /// Whether a line feed starts at `bytes[at]`, written in this encoding.
    pub(crate) fn line_feed_at(self, bytes: &[u8], at: usize) -> bool {
        match self {
            TextCodec::Unicode(TextEncoding::Utf16Le) => {
                bytes.get(at) == Some(&0x0A) && bytes.get(at + 1) == Some(&0x00)
            }
            TextCodec::Unicode(TextEncoding::Utf16Be) => {
                bytes.get(at) == Some(&0x00) && bytes.get(at + 1) == Some(&0x0A)
            }
            TextCodec::Unicode(TextEncoding::Utf8) | TextCodec::Windows1252 => {
                bytes.get(at) == Some(&0x0A)
            }
        }
    }

    fn engine(self) -> &'static encoding_rs::Encoding {
        match self {
            TextCodec::Unicode(TextEncoding::Utf8) => encoding_rs::UTF_8,
            TextCodec::Unicode(TextEncoding::Utf16Le) => encoding_rs::UTF_16LE,
            TextCodec::Unicode(TextEncoding::Utf16Be) => encoding_rs::UTF_16BE,
            TextCodec::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }

    /// `bytes` as text, with a replacement character for anything that is not valid. A byte-order
    /// mark is not looked for: skip it with [`Detected::mark`] first.
    #[must_use]
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        self.engine().decode_without_bom_handling(bytes).0
    }
}

/// How much of a file the bytes handed to [`detect`] are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Coverage {
    /// All of the file.
    Whole,
    /// Its first bytes only, so a character cut off at the end is not an error.
    Prefix,
}

/// The encoding of a file whose first bytes are `head`: a byte-order mark if there is one, else
/// UTF-8 when the bytes are valid UTF-8, else Windows-1252.
#[must_use]
pub fn detect(head: &[u8], coverage: Coverage) -> Detected {
    const MARKS: &[(&[u8], TextEncoding)] = &[
        (&[0xEF, 0xBB, 0xBF], TextEncoding::Utf8),
        (&[0xFF, 0xFE], TextEncoding::Utf16Le),
        (&[0xFE, 0xFF], TextEncoding::Utf16Be),
    ];
    if let Some((mark, encoding)) = MARKS.iter().find(|(mark, _)| head.starts_with(mark)) {
        return Detected {
            codec: TextCodec::Unicode(*encoding),
            mark: mark.len() as u8, // 2 or 3
        };
    }
    let utf8 = match (std::str::from_utf8(head), coverage) {
        (Ok(_), _) => true,
        (Err(error), Coverage::Prefix) => error.error_len().is_none(),
        (Err(_), Coverage::Whole) => false,
    };
    let codec = if utf8 {
        TextCodec::Unicode(TextEncoding::Utf8)
    } else {
        TextCodec::Windows1252
    };
    Detected { codec, mark: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_of_a_file_picks_its_encoding() {
        const UTF8: TextCodec = TextCodec::Unicode(TextEncoding::Utf8);
        const LE: TextCodec = TextCodec::Unicode(TextEncoding::Utf16Le);
        const BE: TextCodec = TextCodec::Unicode(TextEncoding::Utf16Be);
        // name, bytes, coverage, codec, mark
        type Case = (&'static str, &'static [u8], Coverage, TextCodec, u8);
        const CASES: &[Case] = &[
            ("empty", b"", Coverage::Whole, UTF8, 0),
            ("ascii", b"hello", Coverage::Whole, UTF8, 0),
            ("utf-8 text", "héllo".as_bytes(), Coverage::Whole, UTF8, 0),
            ("utf-8 mark", b"\xEF\xBB\xBFhi", Coverage::Whole, UTF8, 3),
            ("utf-16 le mark", b"\xFF\xFEh\0i\0", Coverage::Whole, LE, 2),
            ("utf-16 be mark", b"\xFE\xFF\0h\0i", Coverage::Whole, BE, 2),
            (
                "latin-1 bytes",
                b"caf\xE9",
                Coverage::Whole,
                TextCodec::Windows1252,
                0,
            ),
            (
                "a lone continuation byte",
                b"a\x80b",
                Coverage::Whole,
                TextCodec::Windows1252,
                0,
            ),
            (
                "a character cut off at the end of a prefix",
                b"caf\xC3",
                Coverage::Prefix,
                UTF8,
                0,
            ),
            (
                "the same bytes as a whole file",
                b"caf\xC3",
                Coverage::Whole,
                TextCodec::Windows1252,
                0,
            ),
            (
                "a cut off character before an invalid byte",
                b"\xC3(",
                Coverage::Prefix,
                TextCodec::Windows1252,
                0,
            ),
        ];
        for (name, bytes, coverage, codec, mark) in CASES {
            let want = Detected {
                codec: *codec,
                mark: *mark,
            };
            assert_eq!(detect(bytes, *coverage), want, "{name}");
        }
    }

    #[test]
    fn each_encoding_decodes_its_bytes_and_never_fails() {
        // name, codec, bytes, text
        const CASES: &[(&str, TextCodec, &[u8], &str)] = &[
            (
                "utf-8",
                TextCodec::Unicode(TextEncoding::Utf8),
                "é€".as_bytes(),
                "é€",
            ),
            (
                "utf-8 with a bad byte",
                TextCodec::Unicode(TextEncoding::Utf8),
                b"a\xFFb",
                "a\u{FFFD}b",
            ),
            (
                "utf-16 le",
                TextCodec::Unicode(TextEncoding::Utf16Le),
                b"h\0\xE9\0",
                "hé",
            ),
            (
                "utf-16 be",
                TextCodec::Unicode(TextEncoding::Utf16Be),
                b"\0h\0\xE9",
                "hé",
            ),
            ("windows-1252 euro", TextCodec::Windows1252, b"\x80", "€"),
            (
                "windows-1252 accent",
                TextCodec::Windows1252,
                b"caf\xE9",
                "café",
            ),
        ];
        for (name, codec, bytes, text) in CASES {
            assert_eq!(codec.decode(bytes), *text, "{name}");
        }
    }

    #[test]
    fn labels_and_units_follow_the_encoding() {
        assert_eq!(TextCodec::Windows1252.label(), "Windows-1252");
        assert_eq!(
            TextCodec::Unicode(TextEncoding::Utf16Be).label(),
            "UTF-16 BE"
        );
        assert_eq!(TextCodec::Unicode(TextEncoding::Utf16Le).unit(), 2);
        assert_eq!(TextCodec::Windows1252.unit(), 1);
    }
}
