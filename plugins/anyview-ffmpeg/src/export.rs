//! An export: probe the input, decide the arguments, run ffmpeg writing beside the output under a
//! temporary name, report its progress, and put the file in place only when it is whole.

use crate::encoders::Encoders;
use crate::error::FfmpegError;
use crate::events::{Event, forward_progress, tail_of};
use crate::ffprobe::{Probed, probe};
use crate::keyframe::keyframe_at_or_before;
use crate::plan::{Cut, Plan, plan, range_of};
use crate::target::Target;
use crate::tools::Tools;
use crate::units::Micros;
use anyview_plugin_protocol::{
    Done, ExportRequest, HostMessage, PluginMessage, Progress, write_frame,
};
use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

/// How much of ffmpeg's complaint an error carries.
const COMPLAINT_BYTES: usize = 1500;

/// The name an export is written under until it is whole: beside the output, hidden, unique to
/// this process and this call, and ending as the output does so ffmpeg picks the container from
/// the extension: `.part-<pid>-<n>-<name>`.
pub fn temporary(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .map_or_else(|| OsString::from("export"), std::ffi::OsStr::to_os_string);
    let mut hidden = OsString::from(format!(
        ".part-{}-{}-",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    hidden.push(name);
    output.with_file_name(hidden)
}

/// Which temporary name this process makes next.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Deletes the temporary files beside `output` that a plugin process which is gone left behind
/// (a host that killed it, a crash): `.part-<pid>-<n>-<name>` with no process `<pid>`. Without
/// `/proc` there is no telling, and nothing goes.
fn sweep_dead(output: &Path) {
    if !Path::new("/proc/self").exists() {
        return;
    }
    let Some(dir) = output.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(part_pid) else {
            continue;
        };
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if is_file && !Path::new("/proc").join(pid.to_string()).exists() {
            let _gone = fs::remove_file(entry.path());
        }
    }
}

/// The process that made the temporary file called `name`.
fn part_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(".part-")?;
    let (pid, rest) = rest.split_once('-')?;
    let (number, _) = rest.split_once('-')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    (digits(pid) && digits(number)).then(|| pid.parse().ok())?
}

/// Where the cut of `target` begins: on the video's keyframe at or before the start for a trim
/// of a recording that has pictures, exactly at the start otherwise.
fn cut_for(
    tools: &Tools,
    request: &ExportRequest,
    target: Target,
    probed: &Probed,
) -> Result<Cut, FfmpegError> {
    let (start, _) = range_of(request)?;
    let video = probed
        .video()
        .filter(|_| target == Target::Trim && start > Micros(0));
    let Some(video) = video else {
        return Ok(Cut::at(start));
    };
    let key = keyframe_at_or_before(&tools.ffprobe, &request.input, video.index, start)?;
    Ok(Cut::on_keyframe(key))
}

/// Does `request`. Messages go to `out`; `events` carries the host's `Cancel`, and `sender` is
/// the channel's other end for the thread that reads ffmpeg.
pub fn export(
    tools: &Tools,
    encoders: &Encoders,
    request: &ExportRequest,
    events: &Receiver<Event>,
    sender: &Sender<Event>,
    out: &mut impl Write,
) -> Result<Done, FfmpegError> {
    let target = Target::parse(&request.target).ok_or_else(|| {
        FfmpegError::Unsupported(format!("there is no export target {:?}", request.target))
    })?;
    if !encoders.can_write(target) {
        return Err(FfmpegError::Unsupported(format!(
            "this FFmpeg cannot write {}: it has no encoder for it",
            target.slug()
        )));
    }
    if fs::symlink_metadata(&request.output).is_ok() {
        return Err(FfmpegError::Exists(request.output.clone()));
    }
    let probed = probe(&tools.ffprobe, &request.input)?;
    sweep_dead(&request.output);
    let cut = cut_for(tools, request, target, &probed)?;
    // The name is ours alone: made here, and never a file that was already there.
    let (temp, claimed) = claim_temporary(&request.output)?;
    let finished = plan(request, target, encoders, &probed, cut, &temp)
        .and_then(|planned| run(tools, &planned, &temp, events, sender, out));
    drop(claimed);
    let placed = finished.and_then(|()| place(&temp, &request.output));
    // Ours: made above, and removed on every way out (after a successful `place` it is a second
    // name of the output).
    let _gone = fs::remove_file(&temp);
    placed.map(|()| Done { output: None })
}

/// A temporary file made empty with a name nobody else has.
fn claim_temporary(output: &Path) -> Result<(PathBuf, fs::File), FfmpegError> {
    let temp = temporary(output);
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| FfmpegError::io("make the temporary file", &error))?;
    Ok((temp, file))
}

/// Gives the finished file its name, unless something has that name: a hard link claims it only
/// if no file has it, in one step, where a rename would replace a file made a moment ago. A file
/// system without hard links falls back to looking first, then renaming.
fn place(temp: &Path, output: &Path) -> Result<(), FfmpegError> {
    match fs::hard_link(temp, output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            Err(FfmpegError::Exists(output.to_path_buf()))
        }
        Err(_) if fs::symlink_metadata(output).is_ok() => {
            Err(FfmpegError::Exists(output.to_path_buf()))
        }
        Err(_) => fs::rename(temp, output)
            .map_err(|error| FfmpegError::io("put the output in place", &error)),
    }
}

/// Runs ffmpeg and waits, telling the host how far it has come and killing it on `Cancel`.
fn run(
    tools: &Tools,
    planned: &Plan,
    temp: &Path,
    events: &Receiver<Event>,
    sender: &Sender<Event>,
    out: &mut impl Write,
) -> Result<(), FfmpegError> {
    let mut child = Command::new(&tools.ffmpeg)
        .args(&planned.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| FfmpegError::io("start ffmpeg", &error))?;
    let progress = child.stdout.take();
    let complaints = child.stderr.take();
    let events_in = sender.clone();
    let reader = std::thread::spawn(move || {
        if let Some(progress) = progress {
            forward_progress(progress, &events_in);
        }
    });
    let complaint = std::thread::spawn(move || {
        complaints.map_or_else(String::new, |pipe| tail_of(pipe, COMPLAINT_BYTES))
    });
    let waited = wait(&mut child, planned.total, events, out);
    let _ = child.kill();
    let status = child.wait();
    let _ = reader.join();
    let said = complaint.join().unwrap_or_default();
    waited?;
    match status {
        Ok(status) if status.success() && temp.metadata().is_ok_and(|meta| meta.len() > 0) => {
            if planned.total > Micros(0) {
                send_progress(out, planned.total, planned.total)?;
            }
            Ok(())
        }
        Ok(status) => Err(FfmpegError::Failed(if said.is_empty() {
            format!("ffmpeg ended with {status}")
        } else {
            said
        })),
        Err(error) => Err(FfmpegError::io("wait for ffmpeg", &error)),
    }
}

/// Forwards progress until ffmpeg's pipe closes; a `Cancel`, or a host that has gone, ends it.
fn wait(
    child: &mut Child,
    total: Micros,
    events: &Receiver<Event>,
    out: &mut impl Write,
) -> Result<(), FfmpegError> {
    loop {
        match events.recv() {
            Ok(Event::Progress(done)) => send_progress(out, done, total)?,
            Ok(Event::FfmpegEnded) => return Ok(()),
            Ok(Event::Host(HostMessage::Cancel) | Event::HostGone) | Err(_) => {
                let _ = child.kill();
                return Err(FfmpegError::Cancelled);
            }
            Ok(Event::Host(
                HostMessage::Probe(_)
                | HostMessage::Thumbnail(_)
                | HostMessage::Decode(_)
                | HostMessage::Export(_),
            )) => {}
        }
    }
}

fn send_progress(out: &mut impl Write, done: Micros, total: Micros) -> Result<(), FfmpegError> {
    let done = if total > Micros(0) {
        done.min(total)
    } else {
        done
    };
    write_frame(
        out,
        &PluginMessage::Progress(Progress {
            done: done.0,
            total: total.0,
        }),
        &[],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temporary_name_is_hidden_beside_the_output_unique_and_ends_as_it_does() {
        let first = temporary(Path::new("/music/tone.mp3"));
        let again = temporary(Path::new("/music/tone.mp3"));
        let prefix = format!("/music/.part-{}-", std::process::id());
        assert!(first.to_string_lossy().starts_with(&prefix), "{first:?}");
        assert!(first.to_string_lossy().ends_with("-tone.mp3"), "{first:?}");
        assert_ne!(first, again);
    }

    #[test]
    fn placing_never_replaces_a_file_that_is_there_and_keeps_the_temporary_one_to_the_caller() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.mp3");
        let (temp, _claimed) = claim_temporary(&output).unwrap();
        fs::write(&temp, "new").unwrap();
        fs::write(&output, "theirs").unwrap();
        assert!(matches!(place(&temp, &output), Err(FfmpegError::Exists(_))));
        assert_eq!(fs::read_to_string(&output).unwrap(), "theirs");
        fs::remove_file(&output).unwrap();
        place(&temp, &output).unwrap();
        assert_eq!(fs::read_to_string(&output).unwrap(), "new");
    }

    #[test]
    fn a_dangling_symlink_is_a_taken_output_name() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.mp3");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &output).unwrap();
        let (temp, _claimed) = claim_temporary(&output).unwrap();
        assert!(matches!(place(&temp, &output), Err(FfmpegError::Exists(_))));
        assert!(!dir.path().join("nowhere").exists());
    }

    #[test]
    fn only_the_temporary_files_of_dead_processes_are_swept() {
        if !Path::new("/proc/self").exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out.mp3");
        // 4194304 is above the kernel's largest pid, so no process has it.
        let dead = dir.path().join(".part-4194304-1-out.mp3");
        let live = dir
            .path()
            .join(format!(".part-{}-9-other.mp3", std::process::id()));
        let theirs = dir.path().join(".part-out.mp3");
        for file in [&dead, &live, &theirs] {
            fs::write(file, "x").unwrap();
        }
        sweep_dead(&output);
        assert!(!dead.exists());
        assert!(live.exists());
        assert!(
            theirs.exists(),
            "a file of the person's, named like the old partial one"
        );
    }

    #[test]
    fn a_temporary_file_names_the_process_that_made_it() {
        let cases = [
            ("ours", ".part-4242-7-tone.mp3", Some(4242)),
            ("old name", ".part-tone.mp3", None),
            ("a person's file", ".part-1-tone.mp3", None),
            ("not a number", ".part-x-1-tone.mp3", None),
        ];
        for (name, file, want) in cases {
            assert_eq!(part_pid(file), want, "{name}");
        }
    }
}
