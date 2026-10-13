//! Which encoding a text file's bytes are in, and turning them into text.

use anyview_core::TextEncoding;
use std::borrow::Cow;

/// How many bytes at the start of a file decide its encoding. Text past them that does not fit the
/// choice decodes with replacement characters instead of changing it.
pub const DETECT_BYTES: usize = 1 << 20;

/// An encoding the viewer decodes: what a byte-order mark or valid UTF-8 identifies, or the legacy
/// encoding that `chardetng` guesses for every other file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextCodec {
    /// UTF-8, UTF-16 little endian or UTF-16 big endian.
    Unicode(TextEncoding),
    /// Windows-1252, the web's default for bytes that are not UTF-8 and the guess when the bytes
    /// say nothing else. Every byte maps to a character, so decoding never fails.
    Windows1252,
    /// Another legacy encoding the bytes look like: Shift_JIS, GBK, Big5, EUC-JP, EUC-KR, KOI8-R,
    /// Windows-1251 and the rest of the web's labels. A byte that is not valid in it decodes to a
    /// replacement character.
    Legacy(LegacyEncoding),
}

/// A legacy (non-Unicode) encoding that [`detect`] guessed. It is only made by `detect`, and a
/// guess of Windows-1252 is [`TextCodec::Windows1252`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegacyEncoding(&'static encoding_rs::Encoding);

impl LegacyEncoding {
    /// The encoding's name as the web labels it, such as `Shift_JIS` or `KOI8-R`.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.0.name()
    }
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
    /// `UTF-8`, `UTF-16 LE`, `UTF-16 BE`, `Windows-1252` or the guessed encoding's name (`Shift_JIS`,
    /// `KOI8-R`), as a pane lists it.
    pub fn label(self) -> &'static str {
        match self {
            TextCodec::Unicode(TextEncoding::Utf8) => "UTF-8",
            TextCodec::Unicode(TextEncoding::Utf16Le) => "UTF-16 LE",
            TextCodec::Unicode(TextEncoding::Utf16Be) => "UTF-16 BE",
            TextCodec::Windows1252 => "Windows-1252",
            TextCodec::Legacy(legacy) => legacy.name(),
        }
    }

    /// Bytes per unit the line breaks are written in: 2 for UTF-16, otherwise 1.
    pub(crate) fn unit(self) -> usize {
        match self {
            TextCodec::Unicode(TextEncoding::Utf16Le | TextEncoding::Utf16Be) => 2,
            TextCodec::Unicode(TextEncoding::Utf8)
            | TextCodec::Windows1252
            | TextCodec::Legacy(_) => 1,
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
            // No legacy encoding the detector offers has 0x0A as the second byte of a character
            // (the trail bytes of Shift_JIS, GBK, Big5, EUC-JP and EUC-KR start at 0x40), and
            // ISO-2022-JP is not offered.
            TextCodec::Unicode(TextEncoding::Utf8)
            | TextCodec::Windows1252
            | TextCodec::Legacy(_) => bytes.get(at) == Some(&0x0A),
        }
    }

    fn engine(self) -> &'static encoding_rs::Encoding {
        match self {
            TextCodec::Unicode(TextEncoding::Utf8) => encoding_rs::UTF_8,
            TextCodec::Unicode(TextEncoding::Utf16Le) => encoding_rs::UTF_16LE,
            TextCodec::Unicode(TextEncoding::Utf16Be) => encoding_rs::UTF_16BE,
            TextCodec::Windows1252 => encoding_rs::WINDOWS_1252,
            TextCodec::Legacy(legacy) => legacy.0,
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
/// UTF-8 when the bytes are valid UTF-8, else the legacy encoding `chardetng` guesses from the first
/// [`DETECT_BYTES`] of them (Windows-1252 when they say nothing more).
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
        guess_legacy(head, coverage)
    };
    Detected { codec, mark: 0 }
}

/// Fewer non-ASCII bytes than this are too little for a guess: a word or two with an accent
/// (`café`) reads as Windows-1252, the Western default, rather than whichever Latin encoding
/// the guesser leans to on one byte.
const GUESS_BYTES: usize = 4;

/// The legacy encoding of bytes that are not UTF-8, guessed over at most [`DETECT_BYTES`] of them.
/// The guess is told the stream ended only when `head` is the whole file and all of it was fed.
fn guess_legacy(head: &[u8], coverage: Coverage) -> TextCodec {
    let fed = &head[..head.len().min(DETECT_BYTES)];
    if fed.iter().filter(|byte| !byte.is_ascii()).count() < GUESS_BYTES {
        return TextCodec::Windows1252;
    }
    let ends = matches!(coverage, Coverage::Whole) && fed.len() == head.len();
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(fed, ends);
    let guess = detector.guess(None, chardetng::Utf8Detection::Deny);
    if guess == encoding_rs::WINDOWS_1252 {
        TextCodec::Windows1252
    } else {
        TextCodec::Legacy(LegacyEncoding(guess))
    }
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
                "shift_jis text",
                b"\x82\xB1\x82\xEA\x82\xCD\x93\xFA\x96\x7B\x8C\xEA\x82\xCC\x95\xB6\x8F\xCD\x82\xC5\x82\xB7\x81\x42\x8D\xA1\x93\xFA\x82\xCD\x82\xA2\x82\xA2\x93\x56\x8B\x43\x82\xC5\x82\xB7\x82\xCB\x81\x42\x0A",
                Coverage::Whole,
                TextCodec::Legacy(LegacyEncoding(encoding_rs::SHIFT_JIS)),
                0,
            ),
            (
                "koi8-r text, which chardetng names by its superset KOI8-U",
                b"\xF0\xD2\xC9\xD7\xC5\xD4\x2C\x20\xCD\xC9\xD2\x21\x20\xFC\xD4\xCF\x20\xD2\xD5\xD3\xD3\xCB\xC9\xCA\x20\xD4\xC5\xCB\xD3\xD4\x20\xD7\x20\xCB\xCF\xC4\xC9\xD2\xCF\xD7\xCB\xC5\x20\xEB\xEF\xE9\x2D\x38\x2E\x20\xF3\xC5\xC7\xCF\xC4\xCE\xD1\x20\xC8\xCF\xD2\xCF\xDB\xC1\xD1\x20\xD0\xCF\xC7\xCF\xC4\xC1\x2E\x0A",
                Coverage::Whole,
                TextCodec::Legacy(LegacyEncoding(encoding_rs::KOI8_U)),
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
                "shift_jis",
                TextCodec::Legacy(LegacyEncoding(encoding_rs::SHIFT_JIS)),
                b"\x82\xB1\x82\xF1",
                "こん",
            ),
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
        let koi8 = TextCodec::Legacy(LegacyEncoding(encoding_rs::KOI8_R));
        assert_eq!(koi8.label(), "KOI8-R");
        assert_eq!(koi8.unit(), 1);
    }
}
