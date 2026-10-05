//! Running a tool with a deadline that a cancel also ends.

use crate::error::ToolError;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The most of a tool's output that is kept: its messages, or a picture it writes to stdout.
const KEEP: u64 = 1 << 30;

/// Raised when the host asks to stop or goes away; a running tool is killed when it is.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// A stop not yet raised.
    pub fn new() -> Stop {
        Stop::default()
    }

    /// Raises it.
    pub fn raise(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether it was raised.
    pub fn raised(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a tool that exited well wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// Its standard output.
    pub stdout: Vec<u8>,
}

fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let _ = pipe.by_ref().take(KEEP).read_to_end(&mut kept);
        // Anything past the cap is read and dropped so the tool never blocks on a full pipe.
        let _ = std::io::copy(&mut pipe, &mut std::io::sink());
        kept
    })
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs `command` to its end. It is killed when `limit` passes or `stop` is raised; a tool that
/// exits with a failure status is an error carrying the last of what it said.
pub fn run_tool(mut command: Command, limit: Duration, stop: &Stop) -> Result<Ran, ToolError> {
    let tool = Path::new(command.get_program())
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ToolError::io("start the tool", &error))?;
    let out = child.stdout.take().map(drain);
    let err = child.stderr.take().map(drain);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                kill(&mut child);
                return Err(ToolError::io("wait for the tool", &error));
            }
        }
        if stop.raised() {
            kill(&mut child);
            return Err(ToolError::Cancelled);
        }
        if started.elapsed() >= limit {
            kill(&mut child);
            return Err(ToolError::TimedOut {
                tool,
                seconds: limit.as_secs(),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
    if status.success() {
        Ok(Ran { stdout })
    } else {
        let text = String::from_utf8_lossy(&stderr);
        let message = text
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map_or_else(|| status.to_string(), |line| line.trim().to_owned());
        Err(ToolError::Failed { tool, message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn what_a_tool_writes_is_returned_and_what_it_says_in_failing_is_the_error() {
        let ran = run_tool(sh("printf abc"), Duration::from_secs(5), &Stop::new()).unwrap();
        assert_eq!(ran.stdout, b"abc");
        let error = run_tool(
            sh("echo bad >&2; exit 3"),
            Duration::from_secs(5),
            &Stop::new(),
        )
        .unwrap_err();
        assert!(
            matches!(&error, ToolError::Failed { message, .. } if message == "bad"),
            "{error}"
        );
    }

    #[test]
    fn a_tool_past_its_deadline_or_after_a_stop_is_killed() {
        let marker = tempfile::NamedTempFile::new().unwrap();
        let script = format!("echo $$ > {}; sleep 30", marker.path().display());
        let error = run_tool(sh(&script), Duration::from_millis(300), &Stop::new()).unwrap_err();
        assert!(matches!(error, ToolError::TimedOut { .. }), "{error}");
        assert_dead(marker.path());

        let stop = Stop::new();
        let raiser = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            raiser.raise();
        });
        let error = run_tool(sh(&script), Duration::from_secs(20), &stop).unwrap_err();
        assert!(matches!(error, ToolError::Cancelled), "{error}");
        assert_dead(marker.path());
    }

    fn assert_dead(pid_file: &Path) {
        let pid = std::fs::read_to_string(pid_file).unwrap().trim().to_owned();
        let proc = std::path::PathBuf::from("/proc").join(&pid);
        for _ in 0..50 {
            if !proc.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("process {pid} is still running");
    }
}
