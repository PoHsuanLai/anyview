use super::*;
use anyview_core::work::{Backend, Stop, StopState, Ticket, Ticketed};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);

/// A waker that counts its calls and lets a test block for the next one.
#[derive(Clone)]
struct CountingWaker {
    wakes: Arc<AtomicUsize>,
    sender: Sender<()>,
}

impl UiWaker for CountingWaker {
    fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        let _ = self.sender.send(());
    }
}

fn waker() -> (CountingWaker, Receiver<()>) {
    let (sender, receiver) = channel();
    (
        CountingWaker {
            wakes: Arc::new(AtomicUsize::new(0)),
            sender,
        },
        receiver,
    )
}

/// The jobs a test backend runs.
enum TestJob {
    /// Returns its label.
    Label(&'static str),
    /// Blocks until the gate opens, then returns its label.
    Gated(&'static str, Arc<Mutex<Receiver<()>>>),
    /// Spins until stopped (by flag or deadline) and returns whether it saw the stop.
    UntilStopped,
    Panic,
}

#[derive(Debug, PartialEq, Eq)]
enum TestDone {
    Label(&'static str),
    SawStop,
    /// The worker's id when it ran, to tell a rebuilt scratch from the old one.
    Worker(usize),
}

struct Counting;

impl Backend for Counting {
    type Doc = ();
    type Worker = usize;
    type Job = TestJob;
    type Done = TestDone;

    fn run(_: &(), _: &mut usize, job: TestJob, stop: &Stop) -> TestDone {
        match job {
            TestJob::Label(label) => TestDone::Label(label),
            TestJob::Gated(label, gate) => {
                let _ = gate.lock().map(|gate| gate.recv_timeout(WAIT));
                TestDone::Label(label)
            }
            TestJob::UntilStopped => {
                let started = Instant::now();
                while stop.stopped_at(Instant::now()) == StopState::Running {
                    assert!(started.elapsed() < WAIT, "never stopped");
                    std::thread::yield_now();
                }
                TestDone::SawStop
            }
            TestJob::Panic => panic!("job broke"),
        }
    }
}

/// A backend whose job reports which worker scratch it ran with.
struct Scratchy;

impl Backend for Scratchy {
    type Doc = ();
    type Worker = usize;
    type Job = bool;
    type Done = TestDone;

    fn run(_: &(), worker: &mut usize, job: bool, _: &Stop) -> TestDone {
        if job {
            panic!("scratch is now unknown");
        }
        TestDone::Worker(*worker)
    }
}

type Input = Ticketed<JobOutcome<TestDone>>;

struct Rig {
    pool: Pool,
    mailbox: Mailbox<Input>,
    outbox: Outbox<Input>,
    wakes: Receiver<()>,
    counter: Arc<AtomicUsize>,
}

fn rig(threads: usize) -> Rig {
    let (waker, wakes) = waker();
    let counter = Arc::clone(&waker.wakes);
    let (mailbox, outbox) = Mailbox::new(waker);
    let size = PoolSize::exactly(NonZeroUsize::new(threads).unwrap());
    Rig {
        pool: Pool::new(size).unwrap(),
        mailbox,
        outbox,
        wakes,
        counter,
    }
}

impl Rig {
    fn runner(&self) -> Runner<Counting, Input> {
        Runner::new(&self.pool, self.outbox.clone(), || 0, |done| done)
    }

    /// Collects `count` messages, waiting for wakes between drains.
    fn collect(&self, count: usize) -> Vec<Input> {
        let mut got = Vec::new();
        while got.len() < count {
            got.extend(self.mailbox.drain());
            if got.len() < count {
                self.wakes.recv_timeout(WAIT).unwrap();
            }
        }
        got
    }
}

fn gate() -> (Sender<()>, Arc<Mutex<Receiver<()>>>) {
    let (open, closed) = channel();
    (open, Arc::new(Mutex::new(closed)))
}

fn tickets_and_labels(got: &[Input]) -> Vec<(u64, &'static str)> {
    got.iter()
        .map(|m| match &m.value {
            JobOutcome::Done(TestDone::Label(label)) => (m.ticket.0, *label),
            JobOutcome::Done(TestDone::SawStop | TestDone::Worker(_))
            | JobOutcome::Skipped
            | JobOutcome::Panicked(_) => (m.ticket.0, "other"),
        })
        .collect()
}

#[test]
fn pool_size_leaves_a_core_for_the_ui_and_never_zero() {
    const CASES: &[(usize, usize)] = &[(1, 1), (2, 1), (4, 3), (32, 31)];
    for (cores, expected) in CASES {
        let size = PoolSize::from_cores(NonZeroUsize::new(*cores).unwrap());
        assert_eq!(size.get(), *expected, "{cores} cores");
    }
}

#[test]
fn visible_jobs_run_before_preload_and_each_lane_keeps_its_order() {
    let rig = rig(1);
    let runner = rig.runner();
    let (open, closed) = gate();
    // The single worker is held at the gate while the rest queue up behind it.
    runner.submit(
        Lane::Visible,
        Ticket(0),
        Arc::new(()),
        TestJob::Gated("gate", closed),
        None,
    );
    let queued = [
        (Lane::Preload, 1, "preload-a"),
        (Lane::Preload, 2, "preload-b"),
        (Lane::Visible, 3, "visible-a"),
        (Lane::Visible, 4, "visible-b"),
    ];
    for (lane, ticket, label) in queued {
        runner.submit(
            lane,
            Ticket(ticket),
            Arc::new(()),
            TestJob::Label(label),
            None,
        );
    }
    open.send(()).unwrap();
    let got = rig.collect(5);
    assert_eq!(
        tickets_and_labels(&got),
        [
            (0, "gate"),
            (3, "visible-a"),
            (4, "visible-b"),
            (1, "preload-a"),
            (2, "preload-b")
        ]
    );
}

#[test]
fn stop_before_start_skips_the_work() {
    let rig = rig(1);
    let runner = rig.runner();
    let (open, closed) = gate();
    runner.submit(
        Lane::Visible,
        Ticket(0),
        Arc::new(()),
        TestJob::Gated("gate", closed),
        None,
    );
    let skipped = runner.submit(
        Lane::Visible,
        Ticket(1),
        Arc::new(()),
        TestJob::Label("never"),
        None,
    );
    skipped.cancel();
    assert_eq!(skipped.ticket(), Ticket(1));
    open.send(()).unwrap();
    let got = rig.collect(2);
    assert_eq!(got[0].value, JobOutcome::Done(TestDone::Label("gate")));
    assert_eq!(got[1], Ticketed::new(Ticket(1), JobOutcome::Skipped));
}

#[test]
fn a_running_job_sees_its_cancel() {
    let rig = rig(1);
    let runner = rig.runner();
    let handle = runner.submit(
        Lane::Visible,
        Ticket(5),
        Arc::new(()),
        TestJob::UntilStopped,
        None,
    );
    handle.cancel();
    let got = rig.collect(1);
    // Whether the cancel landed before or after the worker started, the job did not run on.
    assert!(matches!(
        got[0].value,
        JobOutcome::Skipped | JobOutcome::Done(TestDone::SawStop)
    ));
}

#[test]
fn a_deadline_ends_a_running_job() {
    let rig = rig(1);
    let runner = rig.runner();
    let deadline = Instant::now() + Duration::from_millis(30);
    runner.submit(
        Lane::Visible,
        Ticket(1),
        Arc::new(()),
        TestJob::UntilStopped,
        Some(deadline),
    );
    let got = rig.collect(1);
    assert_eq!(got[0].value, JobOutcome::Done(TestDone::SawStop));
    assert!(Instant::now() >= deadline);
}

#[test]
fn a_deadline_already_past_skips_the_job() {
    let rig = rig(1);
    let runner = rig.runner();
    let past = Instant::now();
    runner.submit(
        Lane::Visible,
        Ticket(1),
        Arc::new(()),
        TestJob::Label("late"),
        Some(past),
    );
    let got = rig.collect(1);
    assert_eq!(got[0].value, JobOutcome::Skipped);
}

#[test]
fn results_of_every_ticket_are_delivered_for_the_machine_to_judge() {
    let rig = rig(2);
    let runner = rig.runner();
    for ticket in [3, 1, 2] {
        runner.submit(
            Lane::Visible,
            Ticket(ticket),
            Arc::new(()),
            TestJob::Label("x"),
            None,
        );
    }
    let mut tickets: Vec<u64> = rig.collect(3).iter().map(|m| m.ticket.0).collect();
    tickets.sort_unstable();
    assert_eq!(tickets, [1, 2, 3]);
}

#[test]
fn a_panicking_job_is_reported_and_the_pool_keeps_working() {
    let rig = rig(1);
    let runner = rig.runner();
    runner.submit(Lane::Visible, Ticket(1), Arc::new(()), TestJob::Panic, None);
    runner.submit(
        Lane::Visible,
        Ticket(2),
        Arc::new(()),
        TestJob::Label("after"),
        None,
    );
    let got = rig.collect(2);
    assert_eq!(
        got[0].value,
        JobOutcome::Panicked(JobPanic {
            message: "job broke".to_owned()
        })
    );
    assert_eq!(got[1].value, JobOutcome::Done(TestDone::Label("after")));
}

#[test]
fn a_panic_discards_that_backends_scratch_on_the_worker() {
    let rig = rig(1);
    let made = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&made);
    let runner: Runner<Scratchy, Input> = Runner::new(
        &rig.pool,
        rig.outbox.clone(),
        move || counter.fetch_add(1, Ordering::SeqCst) + 100,
        |done| done,
    );
    runner.submit(Lane::Visible, Ticket(1), Arc::new(()), false, None);
    runner.submit(Lane::Visible, Ticket(2), Arc::new(()), true, None);
    runner.submit(Lane::Visible, Ticket(3), Arc::new(()), false, None);
    let got = rig.collect(3);
    // First and third jobs ran with different scratch: the panic in between threw the first away.
    assert_eq!(got[0].value, JobOutcome::Done(TestDone::Worker(100)));
    assert!(matches!(got[1].value, JobOutcome::Panicked(_)));
    assert_eq!(got[2].value, JobOutcome::Done(TestDone::Worker(101)));
    assert_eq!(made.load(Ordering::SeqCst), 2);
}

#[test]
fn each_worker_thread_builds_its_own_scratch_once() {
    let rig = rig(1);
    let made = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&made);
    let runner: Runner<Scratchy, Input> = Runner::new(
        &rig.pool,
        rig.outbox.clone(),
        move || counter.fetch_add(1, Ordering::SeqCst),
        |done| done,
    );
    for ticket in 0..4 {
        runner.submit(Lane::Visible, Ticket(ticket), Arc::new(()), false, None);
    }
    rig.collect(4);
    assert_eq!(made.load(Ordering::SeqCst), 1);
}

#[test]
fn dropping_the_pool_joins_its_workers_and_drops_queued_jobs() {
    let rig = rig(2);
    let runner = rig.runner();
    let (open, closed) = gate();
    runner.submit(
        Lane::Visible,
        Ticket(0),
        Arc::new(()),
        TestJob::Gated("gate", closed.clone()),
        None,
    );
    runner.submit(
        Lane::Visible,
        Ticket(1),
        Arc::new(()),
        TestJob::Gated("gate", closed),
        None,
    );
    for ticket in 2..8 {
        runner.submit(
            Lane::Preload,
            Ticket(ticket),
            Arc::new(()),
            TestJob::Label("queued"),
            None,
        );
    }
    let Rig {
        pool,
        mailbox,
        counter,
        ..
    } = rig;
    // Release the two running jobs just before the drop joins them.
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let _ = open.send(());
        let _ = open.send(());
    });
    drop(pool);
    release.join().unwrap();
    // Returning at all is the join; the queued preload never ran, so at most the two gated ones
    // delivered.
    assert!(mailbox.drain().len() <= 2);
    assert!(counter.load(Ordering::SeqCst) <= 2);
}

#[test]
fn a_burst_of_posts_wakes_the_ui_once_until_it_drains() {
    let (waker, wakes) = waker();
    let counter = Arc::clone(&waker.wakes);
    let (mailbox, outbox) = Mailbox::<u32>::new(waker);
    for n in 0..5 {
        outbox.send(n);
    }
    wakes.recv_timeout(WAIT).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(mailbox.drain(), [0, 1, 2, 3, 4]);
    outbox.send(5);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert_eq!(mailbox.drain(), [5]);
    assert!(mailbox.drain().is_empty());
}

/// An actor body that owns a `!Sync` value (a `Cell`) and a `!Send` one (an `Rc`).
struct Counter {
    total: std::cell::Cell<i64>,
    _not_send: std::rc::Rc<()>,
    woken: u32,
}

enum CounterCommand {
    Add(i64),
    Panic,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
enum CounterEvent {
    Total(i64),
    Woken(u32),
}

impl ActorBody for Counter {
    type Command = CounterCommand;
    type Event = CounterEvent;

    fn command(&mut self, command: CounterCommand, events: &Outbox<CounterEvent>) -> Flow {
        match command {
            CounterCommand::Add(n) => {
                self.total.set(self.total.get() + n);
                events.send(CounterEvent::Total(self.total.get()));
                Flow::Continue
            }
            CounterCommand::Panic => panic!("actor broke"),
            CounterCommand::Quit => Flow::Quit,
        }
    }

    fn woken(&mut self, events: &Outbox<CounterEvent>) {
        self.woken += 1;
        events.send(CounterEvent::Woken(self.woken));
    }
}

fn counter_actor() -> (Actor<CounterCommand>, Mailbox<CounterEvent>, Receiver<()>) {
    let (waker, wakes) = waker();
    let (mailbox, outbox) = Mailbox::new(waker);
    let actor = Actor::spawn("test-actor", outbox, |_wake| Counter {
        total: std::cell::Cell::new(0),
        _not_send: std::rc::Rc::new(()),
        woken: 0,
    })
    .unwrap();
    (actor, mailbox, wakes)
}

fn next_events(
    mailbox: &Mailbox<CounterEvent>,
    wakes: &Receiver<()>,
    count: usize,
) -> Vec<CounterEvent> {
    let mut got = Vec::new();
    while got.len() < count {
        got.extend(mailbox.drain());
        if got.len() < count {
            wakes.recv_timeout(WAIT).unwrap();
        }
    }
    got
}

#[test]
fn an_actor_applies_commands_in_order_and_posts_events() {
    let (actor, mailbox, wakes) = counter_actor();
    for n in [1, 2, 3] {
        actor.send(CounterCommand::Add(n)).unwrap();
    }
    assert_eq!(
        next_events(&mailbox, &wakes, 3),
        [
            CounterEvent::Total(1),
            CounterEvent::Total(3),
            CounterEvent::Total(6)
        ]
    );
}

#[test]
fn an_actors_wake_reaches_its_body_and_coalesces() {
    let (actor, mailbox, wakes) = counter_actor();
    let wake = actor.waker();
    wake.wake();
    assert_eq!(next_events(&mailbox, &wakes, 1), [CounterEvent::Woken(1)]);
    wake.wake();
    assert_eq!(next_events(&mailbox, &wakes, 1), [CounterEvent::Woken(2)]);
}

#[test]
fn an_actor_that_quits_or_panics_refuses_further_commands() {
    let (actor, _mailbox, _wakes) = counter_actor();
    actor.send(CounterCommand::Quit).unwrap();
    let started = Instant::now();
    while actor.send(CounterCommand::Add(1)).is_ok() {
        assert!(started.elapsed() < WAIT, "actor never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(
        actor.send(CounterCommand::Add(1)),
        Err(RuntimeError::ActorEnded)
    ));

    let (actor, _mailbox, _wakes) = counter_actor();
    actor.send(CounterCommand::Panic).unwrap();
    let started = Instant::now();
    while actor.send(CounterCommand::Add(1)).is_ok() {
        assert!(started.elapsed() < WAIT, "actor never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn dropping_an_actor_joins_its_thread() {
    let (actor, mailbox, wakes) = counter_actor();
    actor.send(CounterCommand::Add(4)).unwrap();
    assert_eq!(next_events(&mailbox, &wakes, 1), [CounterEvent::Total(4)]);
    drop(actor);
    assert!(mailbox.drain().is_empty());
}
