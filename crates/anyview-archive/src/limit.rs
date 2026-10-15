//! Reading inside a budget: a reader that stops at a byte count and remembers that it did.

use anyview_core::work::Interrupt;
use std::cell::Cell;
use std::io::{self, Read, Seek, SeekFrom};
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

/// Bytes read from a reader a chunk at a time.
#[derive(Debug)]
pub(crate) struct Pulled {
    /// What was read.
    pub bytes: Vec<u8>,
    /// Whether the read ended because it was interrupted, not because the reader or the cap did.
    pub interrupted: bool,
}

/// How much is read between two asks of the interrupt.
const PULL_CHUNK: usize = 64 * 1024;

/// At most `cap` + 1 bytes of `reader` (the one over tells a caller the cap was passed), read
/// a chunk at a time so a stream that unpacks slowly is asked about between chunks.
pub(crate) fn pull(
    reader: &mut dyn Read,
    cap: u64,
    interrupt: &dyn Interrupt,
) -> io::Result<Pulled> {
    let want = cap.saturating_add(1);
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; PULL_CHUNK];
    while (bytes.len() as u64) < want {
        if interrupt.interrupted() {
            return Ok(Pulled {
                bytes,
                interrupted: true,
            });
        }
        let room = usize::try_from(want - bytes.len() as u64)
            .map_or(PULL_CHUNK, |left| left.min(PULL_CHUNK));
        match reader.read(&mut chunk[..room]) {
            Ok(0) => break,
            Ok(got) => bytes.extend_from_slice(&chunk[..got]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Pulled {
        bytes,
        interrupted: false,
    })
}
