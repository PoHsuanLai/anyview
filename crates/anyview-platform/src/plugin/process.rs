//! One plugin process and the pipe to it: spawn, read frames with a deadline, write one, kill on
//! drop. Reading polls the pipe instead of blocking on it, so a plugin that stops talking costs
//! a timeout and no thread; the host is single threaded here (CONVENTIONS: the binary owns
//! every thread).

use crate::error::PlatformError;
use anyview_plugin::Installed;
use anyview_plugin_protocol::{
    Frame, FrameDecoder, HostMessage, PROTOCOL_VERSION, PluginMessage, ProtocolError, encode_frame,
};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;
use std::io::{Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// How many of a plugin's last stderr lines an error carries.
const TAIL_LINES: usize = 4;

/// The most one read takes from a pipe (a pipe holds 64 KiB unless the plugin raised it).
const CHUNK: usize = 256 << 10;

/// What the wait for a message came to.
#[derive(Debug)]
pub(crate) enum Arrival {
    /// A message and its payload.
    Message(Frame<PluginMessage>),
    /// Nothing came before the wait ran out.
    Quiet,
}

/// A running plugin. Dropping it kills the process and reaps it.
#[derive(Debug)]
pub(crate) struct PluginProcess {
    id: String,
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
    decoder: FrameDecoder,
    scratch: Box<[u8]>,
    stderr_line: Vec<u8>,
    tail: Vec<String>,
}

impl PluginProcess {
    /// Starts the plugin's program with its manifest arguments, stdin and stdout piped and
    /// stderr captured to the log.
    pub(crate) fn spawn(plugin: &Installed) -> Result<PluginProcess, PlatformError> {
        let id = plugin.manifest.id.as_str().to_owned();
        let program =
            plugin
                .manifest
                .program
                .as_ref()
                .ok_or_else(|| PlatformError::PluginProtocol {
                    plugin: id.clone(),
                    reason: "the manifest has no program".to_owned(),
                })?;
        let mut child = Command::new(&program.path)
            .args(&program.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| PlatformError::Spawn {
                program: program.path.display().to_string(),
                kind: error.kind(),
            })?;
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PlatformError::PluginCrashed {
                plugin: id,
                status: "no standard output".to_owned(),
            });
        };
        Ok(PluginProcess {
            id,
            child,
            stdin,
            stdout,
            stderr,
            decoder: FrameDecoder::new(),
            scratch: vec![0u8; CHUNK].into_boxed_slice(),
            stderr_line: Vec::new(),
            tail: Vec::new(),
        })
    }

    /// The plugin's id, for errors.
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Sends one request.
    pub(crate) fn send(&mut self, message: &HostMessage) -> Result<(), PlatformError> {
        let bytes = encode_frame(message, &[]).map_err(|error| self.protocol(&error))?;
        let written = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(&bytes).and_then(|()| stdin.flush()),
            None => Err(std::io::ErrorKind::BrokenPipe.into()),
        };
        written.map_err(|_| self.crashed())
    }

    /// Waits at most `wait` for the next message. Stderr is read meanwhile and logged.
    pub(crate) fn receive(&mut self, wait: Duration) -> Result<Arrival, PlatformError> {
        let end = Instant::now() + wait;
        loop {
            match self.decoder.next_frame::<PluginMessage>() {
                Ok(Some(frame)) => return Ok(Arrival::Message(frame)),
                Ok(None) => {}
                Err(error) => return Err(self.protocol(&error)),
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(Arrival::Quiet);
            }
            self.pump(left)?;
        }
    }

    /// Waits for readable output for at most `left`, reading what is there.
    fn pump(&mut self, left: Duration) -> Result<(), PlatformError> {
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = vec![PollFd::new(&self.stdout, PollFlags::IN)];
        if let Some(stderr) = &self.stderr {
            fds.push(PollFd::new(stderr, PollFlags::IN));
        }
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => return Ok(()),
            Err(_) => return Err(self.crashed()),
        }
        let ready = |fd: &PollFd<'_>| {
            fd.revents()
                .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
        };
        let out_ready = ready(&fds[0]);
        let err_ready = fds.get(1).is_some_and(ready);
        if err_ready {
            self.read_stderr();
        }
        if out_ready {
            self.read_stdout()?;
        }
        Ok(())
    }

    fn read_stdout(&mut self) -> Result<(), PlatformError> {
        match self.stdout.read(&mut self.scratch) {
            Ok(0) => {
                if self.decoder.is_mid_frame() {
                    return Err(self.protocol(&ProtocolError::Truncated));
                }
                Err(self.crashed())
            }
            Ok(n) => {
                self.decoder.push(&self.scratch[..n]);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(()),
            Err(_) => Err(self.crashed()),
        }
    }

    /// Reads what stderr has and logs each finished line; closes it at the end of the stream.
    fn read_stderr(&mut self) {
        let Some(stderr) = self.stderr.as_mut() else {
            return;
        };
        let mut chunk = [0u8; 4096];
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => {
                self.stderr = None;
                self.finish_line();
            }
            Ok(n) => {
                for byte in &chunk[..n] {
                    if *byte == b'\n' {
                        self.finish_line();
                    } else {
                        self.stderr_line.push(*byte);
                    }
                }
            }
        }
    }

    fn finish_line(&mut self) {
        if self.stderr_line.is_empty() {
            return;
        }
        let line = String::from_utf8_lossy(&self.stderr_line).into_owned();
        self.stderr_line.clear();
        eprintln!("anyview: plugin {}: {line}", self.id);
        self.tail.push(line);
        if self.tail.len() > TAIL_LINES {
            self.tail.remove(0);
        }
    }

    /// The error for a plugin whose output ended: how it exited, and what it last said.
    pub(crate) fn crashed(&mut self) -> PlatformError {
        // Reading stderr to its end lets the last line it wrote before dying show up.
        while self.stderr.is_some() {
            self.read_stderr();
        }
        let exit = self.reap();
        let status = match self.tail.last() {
            Some(line) => format!("{exit}; last said: {line}"),
            None => exit,
        };
        PlatformError::PluginCrashed {
            plugin: self.id.clone(),
            status,
        }
    }

    /// Waits a moment for the process to exit, then kills it, and says how it ended.
    fn reap(&mut self) -> String {
        let end = Instant::now() + Duration::from_millis(200);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status.to_string(),
                Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(5)),
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return "killed after it closed its output".to_owned();
                }
                Err(_) => return "its status could not be read".to_owned(),
            }
        }
    }

    /// A protocol error as the platform's.
    pub(crate) fn protocol(&self, error: &ProtocolError) -> PlatformError {
        self.broke(error.to_string())
    }

    /// The plugin broke the protocol for `reason`.
    pub(crate) fn broke(&self, reason: String) -> PlatformError {
        PlatformError::PluginProtocol {
            plugin: self.id.clone(),
            reason,
        }
    }

    /// The version this viewer speaks.
    pub(crate) const fn supported() -> u32 {
        PROTOCOL_VERSION
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
