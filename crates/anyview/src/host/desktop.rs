//! The desktop, carrying out the tasks the window's requests became. Every platform thing it
//! touches is a trait with a fake, so a test runs the same code against records.

use super::documents;
use super::helpers::HelperHost;
use super::media::{self, Media};
use super::outcome::Outcome;
use super::remembering::{PlaceWriter, REMEMBER_EVERY};
use super::route::Task;
use super::saving;
use super::store::Store;
use super::trash::Trash;
use anyview_core::{FileName, FilePath, Resume, Source, claimed_mimes, opened_globs};
#[cfg(feature = "quire-desktop")]
use anyview_platform::linux::{FileManagerReveal, MailShare, PortalPicker, PortalPrinter};
use anyview_platform::portable::SystemOpen;
#[cfg(not(feature = "quire-desktop"))]
use anyview_platform::portable::{NoPicker, NoPrinter, NoShare, SystemReveal};
use anyview_platform::{
    Env, FileKinds, JobTitle, OpenLink, PickOutcome, Picker, PrintOutcome, Printer, Reveal, Share,
};
use anyview_store::Versions;
use anyview_ui::{DesktopService, HelperEnd, PlatformAbilities, VersionRow};
use ds::prelude::Word;
use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::trash::SystemTrash;

/// What a window's host does: the program's end of the requests, object safe so a window holds
/// one without knowing which platform it runs on.
pub trait Hosting: Send + Sync + 'static {
    /// Start `task` on the program's runtime; the handle resolves when it has ended.
    fn carry_out(&self, task: Task) -> JoinHandle<Outcome>;

    /// Where the person left `source`, if the file is as it was then. Blocking: call it from a
    /// worker.
    fn resume(&self, source: &Source) -> Option<Resume>;

    /// The versions kept of `path`, newest first, as the Revert To sheet lists them. Blocking:
    /// call it from a worker.
    fn kept_versions(&self, path: &FilePath) -> Vec<VersionRow>;

    /// Delete the kept versions older than the keep period. Blocking: the program calls it from
    /// a blocking task once, as it starts.
    fn prune_versions(&self);

    /// The desktop services this platform has, for the windows to offer only what works.
    fn abilities(&self) -> PlatformAbilities;

    /// Write the places still waiting to be kept. Blocking: the program calls it as it ends.
    fn flush(&self);
}

/// The desktop this build runs on: the Linux desktop's services (`quire-desktop`), each over D-Bus or
/// the freedesktop files.
#[cfg(feature = "quire-desktop")]
pub type PlatformDesktop =
    Desktop<FileManagerReveal, MailShare, PortalPrinter, SystemTrash, PortalPicker, SystemOpen>;

/// The desktop this build runs on: the portable one. Links and Show in Folder go through the
/// platform's opener and the trash is the system's; sharing, printing and the file
/// chooser are absent, and each answers "not available".
#[cfg(not(feature = "quire-desktop"))]
pub type PlatformDesktop =
    Desktop<SystemReveal, NoShare, NoPrinter, SystemTrash, NoPicker, SystemOpen>;

/// The platform's parts, shared by every task.
struct Parts<R, S, P, T, F, L> {
    reveal: R,
    share: S,
    printer: P,
    trash: T,
    picker: F,
    links: L,
    store: Arc<Store>,
    versions: Versions,
    remembering: PlaceWriter,
    media: Media,
    helpers: Option<Arc<HelperHost>>,
}

/// What the desktop keeps and plays with besides the platform's traits: the history and view
/// memory, and the players and exports.
#[derive(Debug)]
pub struct Services {
    /// The history and view memory.
    pub store: Store,
    /// The players, the exports and a scratch folder.
    pub media: Media,
    /// The originals kept of every file saved in place.
    pub versions: Versions,
    /// The tools the plugins run, and installing one: absent when the program ships no list of them.
    pub helpers: Option<Arc<HelperHost>>,
}

/// The tasks of every window, carried out on `runtime` through the platform's traits `R`
/// (reveal), `S` (share), `P` (print), `T` (trash), `F` (choose a file) and `L`
/// (open a web link).
pub struct Desktop<R, S, P, T, F, L> {
    runtime: Handle,
    parts: Arc<Parts<R, S, P, T, F, L>>,
}

impl<R, S, P, T, F, L> std::fmt::Debug for Desktop<R, S, P, T, F, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desktop").finish_non_exhaustive()
    }
}

impl<R, S, P, T, F, L> Desktop<R, S, P, T, F, L> {
    /// A desktop made of these parts, running its tasks on `runtime`. One argument for each
    /// platform trait, which is the point of the type: a bundle would only rename them.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        runtime: Handle,
        reveal: R,
        share: S,
        printer: P,
        trash: T,
        picker: F,
        links: L,
        services: Services,
    ) -> Self {
        let Services {
            store,
            media,
            versions,
            helpers,
        } = services;
        let store = Arc::new(store);
        Desktop {
            runtime: runtime.clone(),
            parts: Arc::new(Parts {
                reveal,
                share,
                printer,
                trash,
                picker,
                links,
                remembering: PlaceWriter::new(Arc::clone(&store), runtime.clone(), REMEMBER_EVERY),
                store,
                versions,
                media,
                helpers,
            }),
        }
    }
}

#[cfg(feature = "quire-desktop")]
impl PlatformDesktop {
    /// The Linux desktop of `env`, its store under `store`.
    pub fn platform(runtime: Handle, env: &Env, services: Services) -> PlatformDesktop {
        Desktop::new(
            runtime,
            FileManagerReveal::new(env.clone()),
            MailShare::new(env.clone()),
            PortalPrinter::new(env.clone()),
            SystemTrash,
            PortalPicker::new(env.clone()),
            SystemOpen::new(env.clone()),
            services,
        )
    }
}

#[cfg(not(feature = "quire-desktop"))]
impl PlatformDesktop {
    /// The portable desktop of `env`, its store under `store`.
    pub fn platform(runtime: Handle, env: &Env, services: Services) -> PlatformDesktop {
        Desktop::new(
            runtime,
            SystemReveal::new(env.clone()),
            NoShare,
            NoPrinter,
            SystemTrash,
            NoPicker,
            SystemOpen::new(env.clone()),
            services,
        )
    }
}

impl<R, S, P, T, F, L> Hosting for Desktop<R, S, P, T, F, L>
where
    R: Reveal + Send + Sync + 'static,
    S: Share + Send + Sync + 'static,
    P: Printer + Send + Sync + 'static,
    T: Trash,
    F: Picker + Send + Sync + 'static,
    L: OpenLink + Send + Sync + 'static,
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

    fn kept_versions(&self, path: &FilePath) -> Vec<VersionRow> {
        saving::rows_of(&self.parts.versions, path)
    }

    fn prune_versions(&self) {
        let outcome = saving::prune_versions(&self.parts.versions, self.parts.store.saved_at());
        super::feedback::report(&outcome);
    }

    fn abilities(&self) -> PlatformAbilities {
        let there = [
            (DesktopService::FileChooser, self.parts.picker.present()),
            (DesktopService::PrintDialog, self.parts.printer.present()),
            (
                DesktopService::Sharing,
                !self.parts.share.targets().is_empty(),
            ),
            (DesktopService::FileManager, self.parts.reveal.present()),
        ];
        PlatformAbilities::of(
            there
                .into_iter()
                .filter_map(|(service, present)| present.then_some(service)),
        )
    }

    fn flush(&self) {
        self.parts.remembering.flush();
    }
}

async fn perform<R, S, P, T, F, L>(parts: &Arc<Parts<R, S, P, T, F, L>>, task: Task) -> Outcome
where
    R: Reveal + Send + Sync + 'static,
    S: Share + Send + Sync + 'static,
    P: Printer + Send + Sync + 'static,
    T: Trash,
    F: Picker + Send + Sync + 'static,
    L: OpenLink + Send + Sync + 'static,
{
    match task {
        Task::PickFile => pick(parts).await,
        Task::Provide(helper) => match &parts.helpers {
            Some(helpers) => Outcome::Helped(helper, helpers.provide(helper).await),
            None => Outcome::Helped(helper, HelperEnd::Unsupported(helper.slug().to_owned())),
        },
        Task::OpenLink(uri) => failed("open the link", parts.links.open(&uri)),
        Task::Reveal(file) => failed("reveal the file", parts.reveal.reveal(&file).await),
        Task::Share(file) => share(parts, &file).await,
        Task::Print(probed) => print(parts, probed).await,
        Task::ExportDocument { file, choice } => {
            blocking(move || documents::export(&file, choice)).await
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
        Task::Rename { file, to } => {
            let parts = Arc::clone(parts);
            blocking(move || rename(&parts.versions, &file, &to)).await
        }
        Task::Duplicate(file) => blocking(move || duplicate(&file)).await,
        Task::PlayInBackground(probed) => {
            let media = parts.media.clone();
            blocking(move || media::play_in_background(&media, &probed)).await
        }
        Task::ExportMedia { file, choice } => media::export(&parts.media, &file, choice).await,
        Task::Edit { file, request } => {
            let parts = Arc::clone(parts);
            blocking_save(move || {
                let at = parts.store.saved_at();
                saving::save_edit(&parts.versions, at, &file, request)
            })
            .await
        }
        Task::SaveText { file, save } => {
            let parts = Arc::clone(parts);
            blocking_save(move || {
                let at = parts.store.saved_at();
                saving::save_text(&parts.versions, at, &file, save)
            })
            .await
        }
        Task::Restore { file, version } => {
            let parts = Arc::clone(parts);
            blocking_save(move || {
                let at = parts.store.saved_at();
                saving::restore(&parts.versions, at, &file, &version)
            })
            .await
        }
        Task::RevertTo { file, key } => {
            let parts = Arc::clone(parts);
            blocking_save(move || {
                let at = parts.store.saved_at();
                saving::revert(&parts.versions, at, &file, &key)
            })
            .await
        }
        Task::SaveCopy { file, to } => blocking(move || saving::save_copy(&file, &to)).await,
    }
}

/// `work` on the blocking pool; a panic in it is a failure, not the end of the task.
async fn blocking(work: impl FnOnce() -> Outcome + Send + 'static) -> Outcome {
    match tokio::task::spawn_blocking(work).await {
        Ok(outcome) => outcome,
        Err(error) => Outcome::Failed(format!("a task panicked: {error}")),
    }
}

/// `work`, a save in place, on the blocking pool; a panic in it wrote nothing.
async fn blocking_save(work: impl FnOnce() -> Outcome + Send + 'static) -> Outcome {
    match tokio::task::spawn_blocking(work).await {
        Ok(outcome) => outcome,
        Err(error) => Outcome::NotWritten(format!("the save panicked: {error}")),
    }
}

fn failed<E: Display>(what: &str, result: Result<(), E>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(error) => Outcome::Failed(format!("cannot {what}: {error}")),
    }
}

async fn share<R, S: Share, P, T, F, L>(
    parts: &Parts<R, S, P, T, F, L>,
    file: &FilePath,
) -> Outcome {
    match parts.share.targets().first() {
        Some(target) => failed("share the file", parts.share.share(file, *target).await),
        None => Outcome::Nothing("no way to share a file"),
    }
}

async fn print<R, S, P: Printer, T, F, L>(
    parts: &Parts<R, S, P, T, F, L>,
    probed: anyview_ui::Opened,
) -> Outcome {
    let title = JobTitle(
        probed
            .source
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned()),
    );
    let made = tokio::task::spawn_blocking(move || documents::printout(&probed)).await;
    let bytes = match made {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            return Outcome::Failed(format!("cannot lay the file out to print: {error}"));
        }
        Err(error) => return Outcome::Failed(format!("a task panicked: {error}")),
    };
    match parts.printer.print(&bytes, &title).await {
        Ok(PrintOutcome::Printed | PrintOutcome::Cancelled) => Outcome::Done,
        Ok(PrintOutcome::NoDialog) => Outcome::Nothing("the desktop has no print dialog"),
        Err(error) => Outcome::Failed(format!("cannot print the file: {error}")),
    }
}

/// The files the viewer can show, which the desktop's dialog offers first. One list for every way
/// of asking (Open…, ⌘O and the welcome window's button), made from the same map the desktop
/// entry's `MimeType` line is.
pub(super) fn supported_files() -> FileKinds {
    FileKinds {
        mimes: claimed_mimes()
            .iter()
            .map(|mime| mime.as_str().to_owned())
            .collect(),
        globs: opened_globs()
            .iter()
            .map(|glob| (*glob).to_owned())
            .collect(),
    }
}

/// The files the person chooses in the desktop's dialog.
async fn pick<R, S, P, T, F: Picker, L>(parts: &Parts<R, S, P, T, F, L>) -> Outcome {
    match parts.picker.pick(&supported_files()).await {
        Ok(PickOutcome::Chosen(files)) => Outcome::Picked(files),
        Ok(PickOutcome::Cancelled) => Outcome::Done,
        Ok(PickOutcome::NoDialog) => Outcome::Nothing("the desktop has no file chooser"),
        Err(error) => Outcome::Failed(format!("cannot choose a file: {error}")),
    }
}

/// `file` renamed to `to` beside it, never over a file that is there, and its kept versions
/// follow it.
fn rename(versions: &Versions, file: &FilePath, to: &FileName) -> Outcome {
    let Some(folder) = file.parent() else {
        return Outcome::Failed("cannot rename a root".to_owned());
    };
    let target = folder.as_path().join(to.as_str());
    // The name the person left as it was is no rename, and no clash with itself.
    if target == file.as_path() {
        return Outcome::Done;
    }
    // A symlink is renamed alone: its target, and so its versions, stay where they are.
    let real = std::fs::canonicalize(file.as_path()).ok();
    match anyview_store::rename_noreplace(file.as_path(), &target) {
        Ok(()) => {
            follow(versions, real.as_deref(), &target);
            match FilePath::new(&target) {
                Ok(moved) => Outcome::Moved(moved),
                Err(error) => Outcome::Failed(format!("cannot name the renamed file: {error}")),
            }
        }
        Err(error) if anyview_store::is_taken(&error) => Outcome::Taken,
        Err(error) => Outcome::Failed(format!("cannot rename the file: {error}")),
    }
}

/// The versions of the file that was at `from`, now at `to`, re-keyed to its new path.
fn follow(versions: &Versions, from: Option<&Path>, to: &Path) {
    let (Some(from), Ok(now)) = (from, std::fs::canonicalize(to)) else {
        return;
    };
    if from == now {
        return;
    }
    if let Err(error) = versions.rekey(from, &now) {
        super::feedback::log(&format!(
            "the kept versions did not follow the rename: {error}"
        ));
    }
}

/// A copy of `file` beside it under the first free name, whole or not at all: the copy is made
/// under a hidden name and claims its own only if no file has it, so a failure leaves no
/// truncated copy and a file made meanwhile is never overwritten.
fn duplicate(file: &FilePath) -> Outcome {
    let extension = file
        .as_path()
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_default();
    for _ in 0..DUPLICATE_TRIES {
        let Some(copy) = anyview_store::free_beside(file.as_path(), " copy", &extension) else {
            return Outcome::Failed("cannot find a free name for the copy".to_owned());
        };
        match anyview_store::copy_new(file.as_path(), &copy) {
            Ok(()) => return FilePath::new(&copy).map_or(Outcome::Done, Outcome::Wrote),
            // Someone took the name a moment ago: ask for the next one.
            Err(error) if anyview_store::is_taken(&error) => {}
            Err(error) => return Outcome::Failed(format!("cannot copy the file: {error}")),
        }
    }
    Outcome::Failed("cannot find a free name for the copy".to_owned())
}

/// How many times a copy asks again for a name another program took first.
const DUPLICATE_TRIES: u32 = 16;
