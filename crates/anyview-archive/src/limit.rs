//! Reading inside a budget: a reader that stops at a byte count and remembers that it did.

use std::cell::Cell;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

/// Whether a [`Limited`] reader stopped at its limit; shared with whoever handed the reader to a
/// parser, which reports the parser's own error when the limit was the cause.
#[derive(Debug, Clone, Default)]
pub(crate) struct Hit(Rc<Cell<bool>>);

impl Hit {
    pub(crate) fn happened(&self) -> bool {
        self.0.get()
    }
}

/// A reader that gives at most `left` more bytes and then fails with an error, setting its
/// [`Hit`]. A seek does not spend the budget: skipping over an entry's bytes is free.
#[derive(Debug)]
pub(crate) struct Limited<R> {
    inner: R,
    left: u64,
    hit: Hit,
}

impl<R> Limited<R> {
    pub(crate) fn new(inner: R, limit: u64) -> (Self, Hit) {
        let hit = Hit::default();
        (
            Limited {
                inner,
                left: limit,
                hit: hit.clone(),
            },
            hit,
        )
    }

    fn stop(&self) -> io::Error {
        self.hit.0.set(true);
        io::Error::other("the byte budget is spent")
    }
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            return Err(self.stop());
        }
        let room = usize::try_from(self.left).map_or(buf.len(), |left| left.min(buf.len()));
        let got = self.inner.read(&mut buf[..room])?;
        self.left -= got as u64;
        Ok(got)
    }
}

impl<R: Seek> Seek for Limited<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// A buffer that takes at most `room` more bytes and then fails, setting its [`Hit`]: where a
/// decoder that writes (xz) puts a prefix.
#[derive(Debug)]
pub(crate) struct Capped {
    bytes: Vec<u8>,
    room: usize,
    hit: Hit,
}

impl Capped {
    pub(crate) fn new(room: usize) -> (Self, Hit) {
        let hit = Hit::default();
        (
            Capped {
                bytes: Vec::new(),
                room,
                hit: hit.clone(),
            },
            hit,
        )
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for Capped {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let free = self.room - self.bytes.len();
        if free == 0 && !buf.is_empty() {
            self.hit.0.set(true);
            return Err(io::Error::other("the byte budget is spent"));
        }
        let take = free.min(buf.len());
        self.bytes.extend_from_slice(&buf[..take]);
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
