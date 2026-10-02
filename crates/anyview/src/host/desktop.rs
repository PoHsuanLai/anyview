//! The desktop, carrying out the tasks the window's requests became. Every platform thing it
//! touches is a trait with a fake, so a test runs the same code against records.

use super::media::{self, Media};
use super::outcome::Outcome;
use super::remembering::{REMEMBER_EVERY, Remembering};
use super::route::Task;
use super::store::Store;
use super::trash::Trash;
use anyview_core::{FileName, FilePath, Resume, Source};
use anyview_platform::linux::{DesktopApps, FileManagerReveal, MailShare, PortalPrinter};
use anyview_platform::{AppsForType, Env, JobTitle, PrintOutcome, Printer, Reveal, Share};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::trash::SystemTrash;

/// The prefix of this program's own desktop entries: Open With never offers the viewer to itself.
const OWN_ENTRY: &str = "org.quire.Anyview";

/// What a window's host does: the program's end of the requests, object safe so a window holds
/// one without knowing which platform it runs on.
pub trait Hosting: Send + Sync + 'static {
    /// Start `task` on the program's runtime; the handle resolves when it has ended.
    fn carry_out(&self, task: Task) -> JoinHandle<Outcome>;

    /// Where the person left `source`, if the file is as it was then. Blocking: call it from a
    /// worker.
    fn resume(&self, source: &Source) -> Option<Resume>;

    /// Write the places still waiting to be kept. Blocking: the program calls it as it ends.
    fn flush(&self);
}

/// The Linux desktop.
pub type LinuxDesktop =
    Desktop<DesktopApps, FileManagerReveal, MailShare, PortalPrinter, SystemTrash>;

/// The platform's parts, shared by every task.
struct Parts<A, R, S, P, T> {
    apps: A,
    reveal: R,
    share: S,
    printer: P,
    trash: T,
    store: Arc<Store>,
    remembering: Remembering,
    media: Media,
}

/// What the desktop keeps and plays with besides the platform's traits: the history and view
/// memory, and the players and exports.
#[derive(Debug)]
pub struct Services {
    /// The history and view memory.
    pub store: Store,
    /// The players, the exports and a scratch folder.
    pub media: Media,
}

/// The tasks of every window, carried out on `runtime` through the platform's traits `A`
/// (Open With), `R` (reveal), `S` (share), `P` (print) and `T` (trash).
pub struct Desktop<A, R, S, P, T> {
    runtime: Handle,
    parts: Arc<Parts<A, R, S, P, T>>,
}

impl<A, R, S, P, T> std::fmt::Debug for Desktop<A, R, S, P, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desktop").finish_non_exhaustive()
    }
}

impl<A, R, S, P, T> Desktop<A, R, S, P, T> {
    /// A desktop made of these parts, running its tasks on `runtime`.
    pub fn new(
        runtime: Handle,
        apps: A,
        reveal: R,
        share: S,
        printer: P,
        trash: T,
        services: Services,
    ) -> Self {
        let Services { store, media } = services;
        let store = Arc::new(store);
        Desktop {
            runtime: runtime.clone(),
            parts: Arc::new(Parts {
                apps,
                reveal,
                share,
                printer,
                trash,
                remembering: Remembering::new(Arc::clone(&store), runtime.clone(), REMEMBER_EVERY),
                store,
                media,
            }),
        }
    }
}

impl LinuxDesktop {
    /// The Linux desktop of `env`, its store under `store`.
    pub fn linux(runtime: Handle, env: &Env, services: Services) -> LinuxDesktop {
        Desktop::new(
            runtime,
            DesktopApps::new(env.clone()),
            FileManagerReveal::new(env.clone()),
            MailShare::new(env.clone()),
            PortalPrinter::new(env.clone()),
            SystemTrash,
            services,
        )
    }
}

impl<A, R, S, P, T> Hosting for Desktop<A, R, S, P, T>
where
    A: AppsForType + Send + Sync + 'static,
    R: Reveal + Send + Sync + 'static,
    S: Share + Send + Sync + 'static,
    P: Printer + Send + Sync + 'static,
    T: Trash,
{
    fn carry_out(&self, task: Task) -> JoinHandle<Outcome> {
        let parts = Arc::clone(&self.parts);
        self.runtime
            .spawn(async move { perform(&parts, task).await })
    }

    fn resume(&self, source: &Source) -> Option<Resume> {
        self.parts
            .store
            .resume(source.path(), source.stamp())
            .unwrap_or_default()
    }

    fn flush(&self) {
        self.parts.remembering.flush();
    }
}

async fn perform<A, R, S, P, T>(parts: &Arc<Parts<A, R, S, P, T>>, task: Task) -> Outcome
where
    A: AppsForType + Send + Sync + 'static,
    R: Reveal + Send + Sync + 'static,
    S: Share + Send + Sync + 'static,
    P: Printer + Send + Sync + 'static,
    T: Trash,
{
    match task {
        Task::Reveal(file) => failed("reveal the file", parts.reveal.reveal(&file).await),
        Task::Share(file) => share(parts, &file).await,
        Task::Print(probed) => print(parts, probed.source.path()).await,
        Task::OpenWith(probed) => {
            let parts = Arc::clone(parts);
            blocking(move || open_with(&parts, &probed)).await
        }
        Task::RecordView(probed) => {
            let parts = Arc::clone(parts);
            blocking(move || {
                failed(
                    "record the view",
                    parts
                        .store
                        .viewed(probed.source.path(), probed.sniffed.kind()),
                )
            })
            .await
        }
        Task::Remember { source, resume } => {
            parts.remembering.note(source, resume);
            Outcome::Done
        }
        Task::Trash(file) => {
            let parts = Arc::clone(parts);
            blocking(move || failed("trash the file", parts.trash.trash(&file))).await
        }
        Task::Rename { file, to } => blocking(move || rename(&file, &to)).await,
        Task::Duplicate(file) => blocking(move || duplicate(&file)).await,
        Task::PlayInBackground(probed) => {
            let media = parts.media.clone();
            blocking(move || media::play_in_background(&media, &probed)).await
        }
        Task::ExportMedia { file, choice } => media::export(&parts.media, &file, choice).await,
    }
}

/// `work` on the blocking pool; a panic in it is a failure, not the end of the task.
async fn blocking(work: impl FnOnce() -> Outcome + Send + 'static) -> Outcome {
    match tokio::task::spawn_blocking(work).await {
        Ok(outcome) => outcome,
        Err(error) => Outcome::Failed(format!("a task panicked: {error}")),
    }
}

fn failed<E: Display>(what: &str, result: Result<(), E>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(error) => Outcome::Failed(format!("cannot {what}: {error}")),
    }
}

fn open_with<A: AppsForType, R, S, P, T>(
    parts: &Parts<A, R, S, P, T>,
    probed: &anyview_ui::Probed,
) -> Outcome {
    let others = parts
        .apps
        .apps_for(probed.sniffed.mime())
        .into_iter()
        .find(|entry| !entry.id.as_str().starts_with(OWN_ENTRY));
    match others {
        Some(entry) => failed(
            "open the file with another program",
            parts.apps.open_with(&entry.id, probed.source.path()),
        ),
        None => Outcome::Nothing("no other program opens this type"),
    }
}

async fn share<A, R, S: Share, P, T>(parts: &Parts<A, R, S, P, T>, file: &FilePath) -> Outcome {
    match parts.share.targets().first() {
        Some(target) => failed("share the file", parts.share.share(file, *target).await),
        None => Outcome::Nothing("no way to share a file"),
    }
}

async fn print<A, R, S, P: Printer, T>(parts: &Parts<A, R, S, P, T>, file: &FilePath) -> Outcome {
    let path = file.as_path().to_path_buf();
    let read = tokio::task::spawn_blocking(move || std::fs::read(path)).await;
    let bytes = match read {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            return Outcome::Failed(format!("cannot read the file to print: {error}"));
        }
        Err(error) => return Outcome::Failed(format!("a task panicked: {error}")),
    };
    let title = JobTitle(
        file.file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned()),
    );
    match parts.printer.print(&bytes, &title).await {
        Ok(PrintOutcome::Printed | PrintOutcome::Cancelled) => Outcome::Done,
        Ok(PrintOutcome::NoDialog) => Outcome::Nothing("the desktop has no print dialog"),
        Err(error) => Outcome::Failed(format!("cannot print the file: {error}")),
    }
}

fn rename(file: &FilePath, to: &FileName) -> Outcome {
    let Some(folder) = file.parent() else {
        return Outcome::Failed("cannot rename a root".to_owned());
    };
    let target = folder.as_path().join(to.as_str());
    if target.exists() {
        return Outcome::Failed(format!(
            "cannot rename: {} already exists",
            target.display()
        ));
    }
    match std::fs::rename(file.as_path(), &target) {
        Ok(()) => match FilePath::new(&target) {
            Ok(moved) => Outcome::Moved(moved),
            Err(error) => Outcome::Failed(format!("cannot name the renamed file: {error}")),
        },
        Err(error) => Outcome::Failed(format!("cannot rename the file: {error}")),
    }
}

fn duplicate(file: &FilePath) -> Outcome {
    let Some(copy) = free_copy_of(file.as_path()) else {
        return Outcome::Failed("cannot find a free name for the copy".to_owned());
    };
    failed(
        "copy the file",
        std::fs::copy(file.as_path(), copy).map(drop),
    )
}

/// `name copy.ext`, `name copy 2.ext`, … beside `file`: the first that is free.
fn free_copy_of(file: &Path) -> Option<PathBuf> {
    let folder = file.parent()?;
    let stem = file.file_stem()?.to_string_lossy().into_owned();
    let extension = file
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (1_u32..)
        .map(|n| match n {
            1 => format!("{stem} copy{extension}"),
            n => format!("{stem} copy {n}{extension}"),
        })
        .map(|name| folder.join(name))
        .take(10_000)
        .find(|candidate| !candidate.exists())
}
