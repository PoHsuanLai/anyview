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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

/// How much of ffmpeg's complaint an error carries.
const COMPLAINT_BYTES: usize = 1500;

/// The name an export is written under until it is whole: beside the output, hidden, and ending
/// as the output does so ffmpeg picks the container from the extension.
pub fn temporary(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .map_or_else(|| OsString::from("export"), std::ffi::OsStr::to_os_string);
    let mut hidden = OsString::from(".part-");
    hidden.push(name);
    output.with_file_name(hidden)
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
    if request.output.exists() {
        return Err(FfmpegError::Exists(request.output.clone()));
    }
    let probed = probe(&tools.ffprobe, &request.input)?;
    let temp = temporary(&request.output);
    let cut = cut_for(tools, request, target, &probed)?;
    let planned = plan(request, target, encoders, &probed, cut, &temp)?;
    let _ = fs::remove_file(&temp);
    let finished = run(tools, &planned, &temp, events, sender, out);
    let placed = match finished {
        Ok(()) => place(&temp, &request.output),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    };
    placed.map(|()| Done { output: None })
}

/// Puts the finished file where it was asked for, unless something has appeared there since.
fn place(temp: &Path, output: &Path) -> Result<(), FfmpegError> {
    if output.exists() {
        let _ = fs::remove_file(temp);
        return Err(FfmpegError::Exists(output.to_path_buf()));
    }
    fs::rename(temp, output).map_err(|error| {
        let _ = fs::remove_file(temp);
        FfmpegError::io("put the output in place", &error)
    })
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
    fn the_temporary_name_is_hidden_beside_the_output_and_ends_as_it_does() {
        assert_eq!(
            temporary(Path::new("/music/tone.mp3")),
            PathBuf::from("/music/.part-tone.mp3")
        );
    }
}
