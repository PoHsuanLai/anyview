//! The peek worker: one thread that looks at files for a pane, off the UI thread.
//!
//! A pane asks for a new file on every arrow key, and holding Down through fifty files must peek at
//! the few the worker reaches, never fifty at once (a PDF's first page is the slow one). So the
//! worker holds the latest request only: asking again replaces a request that has not started,
//! and the replaced asker is never answered. This is the behaviour, not a setting.
//!
//! Each peek runs on a thread of its own with a stack large enough for the decoders (they recurse
//! on nested markup and deep documents). A panic there is a failed look and no more. A peek that
//! runs past the overrun is abandoned (its thread finishes or hangs on its own) and the worker goes
//! on to the next file, so one hostile file costs one preview. Once too many abandoned peeks are
//! still running, files are refused unlooked-at until they end.
//!
//! It takes no async runtime: the answer comes to a closure, which sends it wherever the host
//! listens (a channel, a signal's writer).

use crate::any::{AnyPeeked, failed_card, peek_with};
use crate::frames::VideoFrames;
use crate::probe::probe;
use anyview_core::{Input, PeekBudget};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
use std::time::Duration;

/// How a worker runs. The default is what the launcher's pane uses; change one setting with the
/// `with_*` methods. It is `#[non_exhaustive]`, so a setting added later breaks no caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WorkerConfig {
    /// The name of the worker's thread; each peek's thread is named for it too.
    pub name: String,
    /// How long a peek may run before the worker gives up on it. Well past the budget's time,
    /// which the peeks keep to by themselves, so only a decoder that does not return reaches it.
    pub overrun: Duration,
    /// The stack of the thread a peek runs on, in bytes.
    pub stack: usize,
    /// How many abandoned peeks may still be running before the worker stops starting new ones: a
    /// decoder that spins costs a core until it ends, and a folder of such files must not cost
    /// them all.
    pub abandoned: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            name: "anyview-peek".to_owned(),
            overrun: Duration::from_secs(4),
            stack: 8 * 1024 * 1024,
            abandoned: 4,
        }
    }
}

impl WorkerConfig {
    /// This config with the thread named `name`.
    pub fn with_name(self, name: impl Into<String>) -> Self {
        WorkerConfig {
            name: name.into(),
            ..self
        }
    }

    /// This config giving up on a look after `overrun`.
    pub fn with_overrun(self, overrun: Duration) -> Self {
        WorkerConfig { overrun, ..self }
    }

    /// This config running each look on a stack of `stack` bytes.
    pub fn with_stack(self, stack: usize) -> Self {
        WorkerConfig { stack, ..self }
    }

    /// This config refusing files once `abandoned` abandoned looks still run.
    pub fn with_abandoned(self, abandoned: usize) -> Self {
        WorkerConfig { abandoned, ..self }
    }
}

/// What a worker does with a file, blocking: the answer for it.
pub type PeekWork<T> = Arc<dyn Fn(Input) -> T + Send + Sync>;

/// What a worker answers for a file whose look panicked, overran or was refused.
pub type PeekFailure<T> = Arc<dyn Fn(&Input) -> T + Send + Sync>;

/// The request waiting for the worker, and whether its owner is gone.
struct Waiting<T> {
    request: Option<Request<T>>,
    ended: bool,
}

struct Request<T> {
    input: Input,
    reply: Box<dyn FnOnce(T) + Send>,
}

struct Slot<T> {
    waiting: Mutex<Waiting<T>>,
    ready: Condvar,
}

impl<T> Slot<T> {
    fn lock(&self) -> std::sync::MutexGuard<'_, Waiting<T>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A worker thread that looks at the latest file asked for. Dropping it ends the thread.
#[derive(Debug)]
pub struct PeekWorker<T = AnyPeeked> {
    slot: Arc<Slot<T>>,
}

impl<T> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Slot")
    }
}

impl<T: Send + 'static> PeekWorker<T> {
    /// Starts the thread, looking at files as `work` says and answering `failed` for a file whose
    /// look panics, overruns or is refused. Without a thread (the system refused one) every
    /// request waits unanswered rather than the host failing.
    pub fn spawn(config: WorkerConfig, work: PeekWork<T>, failed: PeekFailure<T>) -> Self {
        let slot = Arc::new(Slot {
            waiting: Mutex::new(Waiting {
                request: None,
                ended: false,
            }),
            ready: Condvar::new(),
        });
        let mine = Arc::clone(&slot);
        let _ = std::thread::Builder::new()
            .name(config.name.clone())
            .spawn(move || run(&mine, &config, &work, &failed));
        PeekWorker { slot }
    }

    /// Asks for the look at `input`, replacing the request that has not started: its `reply` is
    /// dropped uncalled, as its answer would have been stale. `reply` runs on the worker's
    /// thread.
    pub fn ask(&self, input: Input, reply: impl FnOnce(T) + Send + 'static) {
        self.slot.lock().request = Some(Request {
            input,
            reply: Box::new(reply),
        });
        self.slot.ready.notify_one();
    }
}

impl PeekWorker<AnyPeeked> {
    /// A worker that probes each file and peeks at it as sniffed, inside `budget`; `frames` is
    /// the host's source of a picture for a video with no cover. A file that cannot be read is an
    /// unavailable card, as `probe` and `peek` make.
    pub fn looking(config: WorkerConfig, budget: PeekBudget, frames: Arc<dyn VideoFrames>) -> Self {
        let work: PeekWork<AnyPeeked> =
            Arc::new(move |input| look_with(input, &budget, frames.as_ref()));
        let failed: PeekFailure<AnyPeeked> =
            Arc::new(|input| failed_card(input, "the preview could not be made"));
        PeekWorker::spawn(config, work, failed)
    }
}

impl<T> Drop for PeekWorker<T> {
    fn drop(&mut self) {
        self.slot.lock().ended = true;
        self.slot.ready.notify_one();
    }
}

/// The look at one file: probed, then peeked as sniffed. A file that cannot be probed is an
/// unavailable card saying why.
pub(crate) fn look_with(input: Input, budget: &PeekBudget, frames: &dyn VideoFrames) -> AnyPeeked {
    match probe(&input) {
        Ok(probed) => peek_with(&probed.input, &probed.sniffed, budget, frames),
        Err(error) => failed_card(&input, &error.to_string()),
    }
}

/// Takes the waiting request, looks, answers; until the worker is dropped.
fn run<T: Send + 'static>(
    slot: &Slot<T>,
    config: &WorkerConfig,
    work: &PeekWork<T>,
    failed: &PeekFailure<T>,
) {
    let abandoned = Arc::new(AtomicUsize::new(0));
    loop {
        let request = {
            let mut waiting = slot.lock();
            loop {
                if waiting.ended {
                    return;
                }
                match waiting.request.take() {
                    Some(request) => break request,
                    None => {
                        waiting = slot
                            .ready
                            .wait(waiting)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                }
            }
        };
        let answer = guarded(config, work, failed, &request.input, &abandoned);
        (request.reply)(answer);
    }
}

/// The look at `input` on a thread of its own: a panic is a failed look, and a look still running
/// after the overrun is abandoned, counted in `abandoned` until its thread ends.
fn guarded<T: Send + 'static>(
    config: &WorkerConfig,
    work: &PeekWork<T>,
    failed: &PeekFailure<T>,
    input: &Input,
    abandoned: &Arc<AtomicUsize>,
) -> T {
    if abandoned.load(Ordering::Acquire) >= config.abandoned {
        return failed(input);
    }
    let (sender, receiver) = mpsc::channel();
    // Whichever of the two (the thread ending, the worker giving up) comes second settles the
    // count: the thread leaves it alone when it finished in time.
    let settled = Arc::new(AtomicBool::new(false));
    let (thread_work, thread_settled, thread_abandoned, thread_input) = (
        Arc::clone(work),
        Arc::clone(&settled),
        Arc::clone(abandoned),
        input.clone(),
    );
    let started = std::thread::Builder::new()
        .name(format!("{}-file", config.name))
        .stack_size(config.stack)
        .spawn(move || {
            let looked = catch_unwind(AssertUnwindSafe(|| thread_work(thread_input)));
            let _ = sender.send(looked.ok());
            if thread_settled.swap(true, Ordering::AcqRel) {
                thread_abandoned.fetch_sub(1, Ordering::AcqRel);
            }
        });
    if started.is_err() {
        return failed(input);
    }
    match receiver.recv_timeout(config.overrun) {
        Ok(Some(answer)) => answer,
        Ok(None) => failed(input),
        Err(_) => {
            if !settled.swap(true, Ordering::AcqRel) {
                abandoned.fetch_add(1, Ordering::AcqRel);
            }
            failed(input)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::FileName;

    fn input(name: &str) -> Input {
        Input::from((
            FileName::new(name).unwrap_or_else(|e| panic!("{e}")),
            Vec::new(),
        ))
    }

    /// What a scripted worker answers, by the file's name.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Said {
        Looked(String),
        Failed(String),
    }

    fn scripted(overrun: Duration, abandoned: usize) -> (PeekWorker<Said>, mpsc::Sender<()>) {
        let (release, hang) = mpsc::channel::<()>();
        let hang = Mutex::new(hang);
        let work: PeekWork<Said> = Arc::new(move |input| match input.name().as_str() {
            "panics" => panic!("a decoder fell over"),
            "hangs" => {
                let _ = hang.lock().unwrap_or_else(PoisonError::into_inner).recv();
                Said::Looked("hung".to_owned())
            }
            other => Said::Looked(other.to_owned()),
        });
        let failed: PeekFailure<Said> =
            Arc::new(|input| Said::Failed(input.name().as_str().to_owned()));
        let config = WorkerConfig::default()
            .with_overrun(overrun)
            .with_abandoned(abandoned);
        (PeekWorker::spawn(config, work, failed), release)
    }

    fn answered(worker: &PeekWorker<Said>, name: &str) -> Said {
        let (sender, receiver) = mpsc::channel();
        worker.ask(input(name), move |said| {
            let _ = sender.send(said);
        });
        receiver
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|e| panic!("no answer: {e}"))
    }

    #[test]
    fn a_waiting_request_is_replaced_by_the_next_and_its_asker_hears_nothing() {
        // No thread: the slot alone, as the worker would find it.
        let worker = PeekWorker::<Said> {
            slot: Arc::new(Slot {
                waiting: Mutex::new(Waiting {
                    request: None,
                    ended: false,
                }),
                ready: Condvar::new(),
            }),
        };
        let (first, heard_first) = mpsc::channel();
        let (second, heard_second) = mpsc::channel();
        worker.ask(input("a"), move |said| {
            let _ = first.send(said);
        });
        worker.ask(input("b"), move |said| {
            let _ = second.send(said);
        });
        let taken = worker.slot.lock().request.take();
        assert_eq!(
            taken
                .as_ref()
                .map(|request| request.input.name().as_str().to_owned()),
            Some("b".to_owned()),
            "the latest request waits"
        );
        assert!(
            matches!(
                heard_first.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ),
            "the replaced asker is told nothing is coming"
        );
        assert!(
            matches!(heard_second.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "the waiting one still waits for its answer"
        );
        drop(taken);
    }

    #[test]
    fn a_file_is_looked_at_and_a_panic_is_a_failed_look_the_next_file_survives() {
        let (worker, _release) = scripted(Duration::from_secs(30), 4);
        assert_eq!(
            answered(&worker, "notes.txt"),
            Said::Looked("notes.txt".to_owned())
        );
        assert_eq!(
            answered(&worker, "panics"),
            Said::Failed("panics".to_owned())
        );
        assert_eq!(answered(&worker, "next"), Said::Looked("next".to_owned()));
    }

    #[test]
    fn a_look_that_overruns_is_abandoned_and_later_files_still_get_looked_at() {
        let (worker, release) = scripted(Duration::from_millis(100), 4);
        assert_eq!(answered(&worker, "hangs"), Said::Failed("hangs".to_owned()));
        assert_eq!(answered(&worker, "next"), Said::Looked("next".to_owned()));
        drop(release);
    }

    #[test]
    fn once_the_abandoned_looks_reach_the_cap_files_are_refused_until_one_ends() {
        let (worker, release) = scripted(Duration::from_millis(50), 1);
        assert_eq!(answered(&worker, "hangs"), Said::Failed("hangs".to_owned()));
        assert_eq!(
            answered(&worker, "next"),
            Said::Failed("next".to_owned()),
            "refused without a look while one hangs"
        );
        release.send(()).unwrap_or_else(|e| panic!("{e}"));
        // The hung look ends and settles the count; the worker looks again.
        let mut said = answered(&worker, "later");
        for _ in 0..100 {
            if said == Said::Looked("later".to_owned()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
            said = answered(&worker, "later");
        }
        assert_eq!(said, Said::Looked("later".to_owned()));
    }

    #[test]
    fn the_default_config_is_the_launchers_four_seconds_and_eight_mebibytes() {
        let config = WorkerConfig::default();
        assert_eq!(config.overrun, Duration::from_secs(4));
        assert_eq!(config.stack, 8 * 1024 * 1024);
        assert_eq!(config.abandoned, 4);
    }
}
