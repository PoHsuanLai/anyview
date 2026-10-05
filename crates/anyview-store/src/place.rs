//! Names beside a file and the one way to claim one: a free name, a copy or a rename that lands
//! on a name only if no file has it, decided by the file system in one step, never by a look
//! followed by a write. A dangling symlink is a taken name. Every program that adds a file next
//! to a person's own (a copy, an export, a rename) goes through here.

use crate::attrs::copy_attributes;
use crate::error::{StoreError, StoreOp};
use crate::io::io_error;
use crate::sweep::sweep_leftovers;
use rustix::fs::{CWD, RenameFlags};
use rustix::io::Errno;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The most names tried before giving up: a folder with that many copies is not one to add to.
const MOST_TRIED: u32 = 10_000;

/// How many bytes one step of a copy moves.
const COPY_STEP: usize = 64 * 1024;

/// Whether nothing at all is called `path`: not a file, not a folder, and not a symlink, even one
/// that points nowhere (creating a file there would create the link's target).
pub fn is_free(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(error) if error.kind() == ErrorKind::NotFound)
}

/// `<stem><suffix>.<extension>` beside `file`, or with ` 2`, ` 3`, ... before the extension: the
/// first that is free. An empty `extension` names a file with none.
pub fn free_beside(file: &Path, suffix: &str, extension: &str) -> Option<PathBuf> {
    let folder = file.parent()?;
    let stem = file.file_stem()?.to_string_lossy().into_owned();
    let dot_extension = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    (1..=MOST_TRIED)
        .map(|n| match n {
            1 => format!("{stem}{suffix}{dot_extension}"),
            n => format!("{stem}{suffix} {n}{dot_extension}"),
        })
        .map(|name| folder.join(name))
        .find(|candidate| is_free(candidate))
}

/// `from` renamed to `to`, which must not exist: a file made there a moment ago is never replaced.
/// The kernel decides (`renameat2` with `RENAME_NOREPLACE`). Where a file system cannot, a hard
/// link claims the name and the old one is removed; a folder, which cannot be linked, is looked
/// for first (the one case with a window left).
pub fn rename_noreplace(from: &Path, to: &Path) -> Result<(), StoreError> {
    match rustix::fs::renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        Err(Errno::EXIST) => Err(exists(to)),
        Err(Errno::INVAL | Errno::NOSYS | Errno::OPNOTSUPP) => rename_by_link(from, to),
        Err(error) => Err(io_error(StoreOp::Rename, from, &error.into())),
    }
}

fn rename_by_link(from: &Path, to: &Path) -> Result<(), StoreError> {
    let is_folder = fs::symlink_metadata(from).is_ok_and(|meta| meta.is_dir());
    if !is_folder {
        match fs::hard_link(from, to) {
            Ok(()) => {
                return fs::remove_file(from)
                    .map_err(|error| io_error(StoreOp::Remove, from, &error));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => return Err(exists(to)),
            Err(_) => {}
        }
    }
    if !is_free(to) {
        return Err(exists(to));
    }
    fs::rename(from, to).map_err(|error| io_error(StoreOp::Rename, from, &error))
}

/// Gives the file `from` the new name `to` and leaves `from` where it is (the caller removes it).
/// A hard link claims `to` only if nothing has it, in one step; a file system without hard links
/// falls back to a no-replace rename, which moves `from` instead.
pub fn link_new(from: &Path, to: &Path) -> Result<(), StoreError> {
    match fs::hard_link(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(exists(to)),
        Err(_) if !is_free(to) => Err(exists(to)),
        Err(_) => rename_noreplace(from, to),
    }
}

/// A copy of the file `from` as the new file `to`, whole or not at all. The bytes go to a hidden
/// file beside `to` and are linked to `to` only when they are all on disk, so a failure leaves no
/// truncated copy, and a `to` that exists, or appears meanwhile, is never touched (the error is
/// `Io` with `AlreadyExists`). It keeps the mode, the owner where it may, and the extended
/// attributes and ACLs.
pub fn copy_new(from: &Path, to: &Path) -> Result<(), StoreError> {
    let dir = to.parent().unwrap_or_else(|| Path::new("."));
    sweep_leftovers(dir);
    let mut source = fs::File::open(from).map_err(|e| io_error(StoreOp::Read, from, &e))?;
    let meta = source
        .metadata()
        .map_err(|e| io_error(StoreOp::Stat, from, &e))?;
    let partial = partial_beside(to);
    let mut copy = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&partial)
        .map_err(|e| io_error(StoreOp::Write, &partial, &e))?;
    let copied = copy_bytes(&mut source, &mut copy).and_then(|()| {
        copy_attributes(&source, &copy, &meta);
        copy.sync_all()
    });
    drop(copy);
    let placed = copied
        .map_err(|e| io_error(StoreOp::Write, &partial, &e))
        .and_then(|()| link_new(&partial, to));
    let _gone = fs::remove_file(&partial);
    placed
}

fn copy_bytes(source: &mut fs::File, copy: &mut fs::File) -> io::Result<()> {
    use io::{Read, Write};
    let mut step = vec![0_u8; COPY_STEP];
    loop {
        let read = source.read(&mut step)?;
        if read == 0 {
            return Ok(());
        }
        copy.write_all(&step[..read])?;
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// `<pid>-<n>`: unique among every temporary name this program makes, and tells a sweep which
/// process to ask whether it is still running.
fn unique() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// The hidden name bytes are written under before `to` has them: `.<name>.<pid>-<n>.part` beside
/// it. No two calls, in this process or another, return the same name.
pub fn partial_beside(to: &Path) -> PathBuf {
    hidden_beside(to, "", "part")
}

/// The hidden name a save writes its bytes under before they replace `target`.
pub(crate) fn save_temp_beside(target: &Path) -> PathBuf {
    hidden_beside(target, "anyview-", "tmp")
}

fn hidden_beside(to: &Path, tag: &str, extension: &str) -> PathBuf {
    let name = to
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    to.with_file_name(format!(".{name}.{tag}{}.{extension}", unique()))
}

fn exists(path: &Path) -> StoreError {
    io_error(StoreOp::Rename, path, &ErrorKind::AlreadyExists.into())
}

/// Whether `error` says the name was taken.
pub fn is_taken(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::Io {
            kind: ErrorKind::AlreadyExists,
            ..
        }
    )
}
