//! `FileHead`: the first bytes of a file, which is all sniffing reads.

use crate::source::ByteLen;

/// The first 4 KiB of a file, read by the caller. A longer slice is cut to its first 4 KiB, so
/// sniffing never depends on more than a caller could afford to read before opening a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHead<'a>(&'a [u8]);

impl<'a> FileHead<'a> {
    /// The most bytes a head holds.
    pub const MAX: ByteLen = ByteLen(4096);

    /// The first 4 KiB of `bytes`, or all of them when there are fewer.
    pub fn new(bytes: &'a [u8]) -> Self {
        let max = usize::try_from(Self::MAX.0).unwrap_or(usize::MAX);
        FileHead(&bytes[..bytes.len().min(max)])
    }

    /// The bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.0
    }

    /// Whether the head is as long as it can be, so the file may continue past it and its last
    /// character may be cut in half.
    pub fn is_full(&self) -> bool {
        u64::try_from(self.0.len()).is_ok_and(|len| len >= Self::MAX.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_keeps_at_most_four_kibibytes() {
        const CASES: &[(&str, usize, usize, bool)] = &[
            // name, input length, head length, full
            ("empty", 0, 0, false),
            ("short", 10, 10, false),
            ("one under", 4095, 4095, false),
            ("exact", 4096, 4096, true),
            ("over", 10_000, 4096, true),
        ];
        for (name, input, kept, full) in CASES {
            let bytes = vec![b'a'; *input];
            let head = FileHead::new(&bytes);
            assert_eq!(head.bytes().len(), *kept, "{name}");
            assert_eq!(head.is_full(), *full, "{name}");
        }
    }
}
