//! Single instance without D-Bus: the first viewer is the user's `anyview` agent (latchkey: an
//! advisory lock plus a Unix socket or a named pipe), and a later launch connects, hands over
//! its request as one line (`frame`) and leaves. Where the person has no session bus (macOS,
//! Windows, a desktop with none) this is the whole of single instance.
//!
//! Unlike the bus, nothing starts the viewer when a request arrives and none runs: the launch
//! that finds nobody home is the viewer itself.

use super::frame::{self, FrameError, MAX_LINE};
use crate::error::PlatformError;
use crate::instance::{Claim, Instance, Primary, Request};
use latchkey::{Agent, Error as Latch, Listening, Stream};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// The agent's name: it becomes a directory under the runtime directory, and part of a pipe name
/// on Windows.
const AGENT: &str = "anyview";
/// How long a launch waits for a viewer that holds the lock to open its door.
const DOOR: Duration = Duration::from_secs(3);
/// How long a launch waits for the whole exchange before it gives up and runs on alone.
const EXCHANGE: Duration = Duration::from_secs(10);
/// How long the accept loop rests after the door fails, so a failing door does not spin.
const REST: Duration = Duration::from_millis(100);

/// Where the agent's socket and lock go.
#[derive(Debug, Clone)]
enum Place {
    /// By the platform's own rules: `$XDG_RUNTIME_DIR`, `$TMPDIR` on macOS, a named pipe on
    /// Windows.
    Process,
    /// Everything inside this directory, for tests.
    Under(PathBuf),
}

/// [`Instance`] over latchkey's per-user socket.
#[derive(Debug, Clone)]
pub struct LatchkeyInstance {
    place: Place,
}

impl LatchkeyInstance {
    /// The viewer's agent where the platform puts one.
    pub fn new() -> Self {
        LatchkeyInstance {
            place: Place::Process,
        }
    }

    /// The viewer's agent with its socket and lock inside `runtime`, and nothing outside it: for
    /// tests, which must never touch the person's runtime directory.
    pub fn under(runtime: PathBuf) -> Self {
        LatchkeyInstance {
            place: Place::Under(runtime),
        }
    }

    fn agent(&self) -> Result<Agent, PlatformError> {
        match &self.place {
            Place::Process => Agent::new(AGENT),
            Place::Under(dir) => {
                let dir = dir.as_os_str();
                // A Windows pipe name is machine-wide, so it carries a name taken from the
                // directory: two scratch directories are two agents there too.
                let user = dir.to_string_lossy().chars().fold(0u64, |hash, c| {
                    hash.wrapping_mul(31).wrapping_add(u64::from(c))
                });
                let user = format!("scratch{user:x}");
                let environment = latchkey::Environment {
                    runtime_dir: Some(dir),
                    tmpdir: Some(dir),
                    home: Some(dir),
                    local_app_data: Some(dir),
                    user: Some(&user),
                };
                Agent::in_environment(AGENT, latchkey::here(), &environment)
            }
        }
        .map_err(|error| failed("find the viewer's socket", &error))
    }
}

impl Default for LatchkeyInstance {
    fn default() -> Self {
        LatchkeyInstance::new()
    }
}

fn failed(call: &'static str, error: &dyn std::fmt::Display) -> PlatformError {
    PlatformError::bus(call, error)
}

impl Instance for LatchkeyInstance {
    async fn claim(&self, request: &Request) -> Result<Claim, PlatformError> {
        let agent = self.agent()?;
        let request = request.clone();
        let (sender, requests) = channel();
        let settled = tokio::time::timeout(
            EXCHANGE,
            tokio::task::spawn_blocking(move || settle(agent, sender, &request)),
        )
        .await
        .map_err(|_| failed("reach the running viewer", &"it did not answer in time"))?
        .map_err(|error| failed("claim the viewer's socket", &error))??;
        Ok(match settled {
            Settled::Primary(door) => Claim::Primary(Primary::new(requests, Box::new(door))),
            Settled::Forwarded => Claim::Forwarded,
        })
    }
}

enum Settled {
    Primary(Door),
    Forwarded,
}

/// Take the lock and open the door, or hand `request` to the viewer that has them.
fn settle(
    agent: Agent,
    sender: Sender<Request>,
    request: &Request,
) -> Result<Settled, PlatformError> {
    match agent.listen() {
        Ok(listening) => Ok(Settled::Primary(Door::open(agent, listening, sender))),
        Err(Latch::AlreadyRunning) => forward(&agent, request).map(|()| Settled::Forwarded),
        Err(error) => Err(failed("claim the viewer's socket", &error)),
    }
}

/// Send `request` to the running viewer and wait for it to say it took it.
fn forward(agent: &Agent, request: &Request) -> Result<(), PlatformError> {
    // The viewer holds the lock a moment before its door is open; knocking again covers it. Nothing
    // is started here: the closure is the point where latchkey would launch a viewer.
    let mut stream = agent
        .connect_or_start(|| Ok(()), DOOR)
        .map_err(|error| failed("connect to the running viewer", &error))?;
    let mut line = frame::encode(request);
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|error| failed("send the request to the running viewer", &error))?;
    let mut reply = String::new();
    BufReader::new(stream)
        .take(MAX_LINE)
        .read_line(&mut reply)
        .map_err(|error| failed("hear back from the running viewer", &error))?;
    frame::understand(&reply).map_err(|why| failed("hand the request to the running viewer", &why))
}

/// The open door: the accept loop and everything that keeps the name. Dropping it stops the
/// loop and removes the socket, then lets go of the lock.
#[derive(Debug)]
struct Door {
    agent: Agent,
    stopping: Arc<AtomicBool>,
    accepting: Option<JoinHandle<()>>,
}

impl Door {
    fn open(agent: Agent, listening: Listening, sender: Sender<Request>) -> Door {
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let accepting = std::thread::Builder::new()
            .name("anyview-instance".to_owned())
            .spawn(move || accept_loop(&listening, &stop, &sender))
            .ok();
        Door {
            agent,
            stopping,
            accepting,
        }
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // The loop is blocked in `accept`: one knock wakes it to see the flag.
        drop(self.agent.connect());
        if let Some(accepting) = self.accepting.take() {
            drop(accepting.join());
        }
    }
}

/// Take each client in turn until told to stop. `listening` is dropped when this ends, which
/// removes the socket.
fn accept_loop(listening: &Listening, stopping: &AtomicBool, sender: &Sender<Request>) {
    loop {
        let accepted = listening.accept();
        if stopping.load(Ordering::SeqCst) {
            return;
        }
        match accepted {
            Ok(stream) => {
                let sender = sender.clone();
                // A thread per client, so one that never finishes its line holds up nobody.
                let started = std::thread::Builder::new()
                    .name("anyview-instance-client".to_owned())
                    .spawn(move || serve(stream, &sender));
                if let Err(error) = started {
                    eprintln!("anyview: cannot serve a launch: {error}");
                }
            }
            Err(_) => std::thread::sleep(REST),
        }
    }
}

/// Read one request from `stream`, hand it to the viewer and say whether it was taken.
fn serve(stream: Stream, sender: &Sender<Request>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = (&mut reader).take(MAX_LINE).read_line(&mut line);
    let result = match read {
        Ok(_) if line.ends_with('\n') => frame::decode(line.trim_end())
            .and_then(|request| sender.send(request).map_err(|_| FrameError::Closing)),
        Ok(_) => Err(FrameError::CutShort),
        Err(error) => Err(FrameError::Unreadable(error.to_string())),
    };
    let mut answer = frame::answer(&result);
    answer.push('\n');
    let stream = reader.get_mut();
    drop(
        stream
            .write_all(answer.as_bytes())
            .and_then(|()| stream.flush()),
    );
}
