//! Files, fakes and a desktop made of them.

use crate::host::{Clock, Desktop, Store, Trash, TrashError};
use anyview_core::{
    ByteLen, FileHead, FileName, FilePath, FileStamp, ModTime, SniffStep, Source, sniff,
};
use anyview_platform::testing::{FakeApps, FakePrinter, FakeReveal, FakeShare};
use anyview_platform::{AppEntry, Association, DesktopId, PrintOutcome};
use anyview_store::Viewed;
use anyview_ui::{Probed, StageFamily, family_of};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

/// The time every view is stamped with.
pub const NOW: Viewed = Viewed(1_700_000_000);

pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
pub const PDF: &[u8] = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n";

pub fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

/// `name` holding `bytes`, as the probe of a window would find it.
pub fn probed(at: &Path, name: &str, bytes: &[u8]) -> Probed {
    std::fs::write(at.join(name), bytes).unwrap();
    let file = FileName::new(name).unwrap();
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(bytes), &file) else {
        panic!("{name} needs a look inside");
    };
    let stamp = FileStamp {
        len: ByteLen(bytes.len() as u64),
        modified: ModTime(1),
    };
    let family: StageFamily = family_of(sniffed.kind());
    Probed {
        source: Source::new(FilePath::new(at.join(name)).unwrap(), stamp),
        sniffed,
        family,
    }
}

/// A trash that remembers what was trashed.
#[derive(Debug, Clone, Default)]
pub struct FakeTrash(Arc<Mutex<Vec<FilePath>>>);

impl FakeTrash {
    pub fn trashed(&self) -> Vec<FilePath> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Trash for FakeTrash {
    fn trash(&self, file: &FilePath) -> Result<(), TrashError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(file.clone());
        Ok(())
    }
}

/// The fakes a desktop is made of, kept to read what they were asked.
pub struct Fakes {
    pub apps: FakeApps,
    pub reveal: FakeReveal,
    pub share: FakeShare,
    pub printer: FakePrinter,
    pub trash: FakeTrash,
}

pub type TestDesktop = Desktop<FakeApps, FakeReveal, FakeShare, FakePrinter, FakeTrash>;

pub fn entry(id: &str) -> AppEntry {
    AppEntry {
        id: DesktopId::new(id).unwrap(),
        name: id.to_owned(),
        association: Association::Default,
    }
}

/// A desktop over fakes on the current runtime, its store under `scratch`.
pub fn desktop(scratch: &Path, apps: Vec<AppEntry>) -> (TestDesktop, Fakes) {
    let fakes = Fakes {
        apps: FakeApps::offering(apps),
        reveal: FakeReveal::default(),
        share: FakeShare::default(),
        printer: FakePrinter::answering(PrintOutcome::Printed),
        trash: FakeTrash::default(),
    };
    let now: Clock = Arc::new(|| NOW);
    let desktop = Desktop::new(
        tokio::runtime::Handle::current(),
        fakes.apps.clone(),
        fakes.reveal.clone(),
        fakes.share.clone(),
        fakes.printer.clone(),
        fakes.trash.clone(),
        Store::new(&scratch.join("store"), now),
    );
    (desktop, fakes)
}
