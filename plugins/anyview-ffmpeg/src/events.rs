//! What the main loop waits on: the host's messages and the running ffmpeg's progress, which
//! threads of their own turn into one stream of events so a `Cancel` is seen at once.

use crate::units::Micros;
use anyview_plugin_protocol::{HostMessage, read_frame};
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::Sender;

/// Something that happened.
#[derive(Debug)]
pub enum Event {
    /// The host sent a message.
    Host(HostMessage),
    /// The host closed its end: nobody is waiting for the answer any more.
    HostGone,
    /// ffmpeg says this much of the output is written.
    Progress(Micros),
    /// ffmpeg closed its progress pipe, which it does when it exits.
    FfmpegEnded,
}

/// Reads the host's messages from `input` until it ends.
pub fn forward_host(mut input: impl Read, events: &Sender<Event>) {
    while let Ok(frame) = read_frame::<_, HostMessage>(&mut input) {
        if events.send(Event::Host(frame.message)).is_err() {
            return;
        }
    }
    let _ = events.send(Event::HostGone);
}

/// Reads ffmpeg's `-progress` lines (`key=value`) and sends the time written.
pub fn forward_progress(output: impl Read, events: &Sender<Event>) {
    for line in BufReader::new(output).lines().map_while(Result::ok) {
        if let Some(time) = progress_time(&line) {
            if events.send(Event::Progress(time)).is_err() {
                return;
            }
        }
    }
    let _ = events.send(Event::FfmpegEnded);
}

/// The time in an `out_time_us=1032000` line; none for any other line or for `N/A`.
pub fn progress_time(line: &str) -> Option<Micros> {
    line.strip_prefix("out_time_us=")?
        .trim()
        .parse()
        .ok()
        .map(Micros)
}

/// The last `keep` bytes of what `source` writes, read to its end: ffmpeg's complaints, for an
/// error message, without ever leaving its pipe full.
pub fn tail_of(mut source: impl Read, keep: usize) -> String {
    let mut tail = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(read) = source.read(&mut chunk) {
        if read == 0 {
            break;
        }
        tail.extend_from_slice(&chunk[..read]);
        if tail.len() > keep {
            tail.drain(..tail.len() - keep);
        }
    }
    String::from_utf8_lossy(&tail).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_microsecond_line_is_progress() {
        assert_eq!(
            progress_time("out_time_us=1032000"),
            Some(Micros(1_032_000))
        );
        assert_eq!(progress_time("out_time_us=N/A"), None);
        assert_eq!(progress_time("out_time_us=-5"), None);
        assert_eq!(progress_time("out_time=00:00:01.0"), None);
        assert_eq!(progress_time("progress=end"), None);
    }

    #[test]
    fn the_tail_keeps_the_last_bytes() {
        assert_eq!(tail_of(&b"abcdefgh"[..], 3), "fgh");
        assert_eq!(tail_of(&b"  hi \n"[..], 100), "hi");
    }
}
