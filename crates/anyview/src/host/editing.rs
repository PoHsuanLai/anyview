//! Edits, undo, revert and Save a Copy as the host decides them. Pure, like routing: the trail of
//! the file shown says whether a save may start, and the answer is the task that does it.

use super::outcome::{Declined, Outcome};
use super::route::{Carry, Shown, Task};
use anyview_core::{FileName, FilePath, Trail, TrailIn, TrailOut, edits_for};
use anyview_store::VersionId;
use anyview_ui::{EditRequest, HostRequest, Rewind, TypedText, VersionKey};
use std::path::Path;

impl Shown {
    /// The window after a task ended: a save that kept a version, or one that did not write, moves
    /// its trail; every other outcome leaves it.
    pub fn after(self, outcome: &Outcome) -> Shown {
        let input = match outcome {
            Outcome::Written { file: _, kept } => TrailIn::Kept(kept.clone()),
            // What was waiting for the save asked about a file that is as it was: not any more.
            Outcome::NotWritten(_) => {
                return Shown {
                    queued: Vec::new(),
                    ..self.stepped(TrailIn::Failed).0
                };
            }
            Outcome::Done
            | Outcome::Moved(_)
            | Outcome::Wrote(_)
            | Outcome::Picked(_)
            | Outcome::Taken
            | Outcome::Nothing(_)
            | Outcome::Handed
            | Outcome::Helped(_, _)
            | Outcome::Failed(_) => return self,
        };
        self.stepped(input).0
    }

    /// The trail stepped on `input`, and what it wants done.
    fn stepped(self, input: TrailIn<VersionId>) -> (Shown, Vec<TrailOut<VersionId>>) {
        let Shown {
            file,
            trail,
            queued,
        } = self;
        let (trail, outs) = trail.after(input);
        (
            Shown {
                file,
                trail,
                queued,
            },
            outs,
        )
    }

    /// The first request that waited for a save, and the window after it: asked again once the
    /// save that held it up has ended.
    pub fn next_queued(self) -> (Shown, Option<HostRequest>) {
        let Shown {
            file,
            trail,
            mut queued,
        } = self;
        let next = (!queued.is_empty()).then(|| queued.remove(0));
        (
            Shown {
                file,
                trail,
                queued,
            },
            next,
        )
    }

    /// `request` waits for the save being written.
    fn queueing(mut self, request: HostRequest) -> (Shown, Carry) {
        self.queued.push(request);
        (self, Carry::Declined(Declined::Queued))
    }

    /// The trail as it is now.
    pub fn trail(&self) -> &Trail<VersionId> {
        &self.trail
    }
}

/// An edit the window asked for: saved in place when the file is of a kind that has it and no
/// other save is being written.
pub(super) fn edit(shown: Shown, request: EditRequest) -> (Shown, Carry) {
    let Some(probed) = shown.file().cloned() else {
        return declined(shown, Declined::NoFileShown);
    };
    if !edits_for(probed.sniffed.kind()).contains(&request.edit.kind()) {
        return declined(shown, Declined::Edit);
    }
    if anyview_store::is_read_only(probed.source.path().as_path()) {
        return declined(shown, Declined::Locked);
    }
    let (shown, outs) = shown.stepped(TrailIn::Save);
    if is_busy(&outs) {
        return shown.queueing(HostRequest::Edit(request));
    }
    saves(shown, outs, || Task::Edit {
        file: probed,
        request,
    })
}

/// Undo or redo: the version the trail names is put back.
pub(super) fn rewind(shown: Shown, rewind: Rewind) -> (Shown, Carry) {
    let Some(file) = shown.file().map(|probed| probed.source.path().clone()) else {
        return declined(shown, Declined::NoFileShown);
    };
    if anyview_store::is_read_only(file.as_path()) {
        return declined(shown, Declined::Locked);
    }
    let input = match rewind {
        Rewind::Undo => TrailIn::Undo,
        Rewind::Redo => TrailIn::Redo,
    };
    let (shown, outs) = shown.stepped(input);
    if is_busy(&outs) {
        return shown.queueing(HostRequest::Rewind(rewind));
    }
    restores(shown, outs, |version| Task::Restore { file, version })
}

/// Revert To: the kept version `key` names is put back, and the file as it is now is kept too.
pub(super) fn revert(shown: Shown, key: VersionKey) -> (Shown, Carry) {
    let Some(file) = shown.file().map(|probed| probed.source.path().clone()) else {
        return declined(shown, Declined::NoFileShown);
    };
    if anyview_store::is_read_only(file.as_path()) {
        return declined(shown, Declined::Locked);
    }
    let (shown, outs) = shown.stepped(TrailIn::Save);
    if is_busy(&outs) {
        return shown.queueing(HostRequest::RevertTo(key));
    }
    saves(shown, outs, || Task::RevertTo { file, key })
}

/// Save a Copy: the file written at the place the typed text names.
pub(super) fn save_copy(shown: Shown, typed: TypedText) -> (Shown, Carry) {
    let Some(file) = shown.file().map(|probed| probed.source.path().clone()) else {
        return declined(shown, Declined::NoFileShown);
    };
    match destination(&file, typed.as_str()) {
        Some(to) => (shown, Carry::Desktop(Task::SaveCopy { file, to })),
        None => declined(shown, Declined::NotAFileName),
    }
}

/// Where a copy of `file` goes when `typed` is what the person wrote: a path as it is when it is
/// absolute, otherwise a name beside the file. `None` for text that is neither.
fn destination(file: &FilePath, typed: &str) -> Option<FilePath> {
    let text = typed.trim();
    if Path::new(text).is_absolute() {
        return FilePath::new(text).ok();
    }
    let name = FileName::new(text).ok()?;
    FilePath::new(file.parent()?.as_path().join(name.as_str())).ok()
}

/// Whether the trail refused for a save in flight.
fn is_busy(outs: &[TrailOut<VersionId>]) -> bool {
    matches!(outs.first(), Some(TrailOut::Busy) | None)
}

/// What the trail allowed.
enum Answer {
    /// A save may go ahead.
    Save,
    /// This version is to be put back.
    Restore(VersionId),
}

/// What the trail answered, or why it did not allow the request. Every input the host gives a
/// trail answers with one output, so an empty answer cannot come, and is counted as a busy trail
/// if it does.
fn answered(outs: Vec<TrailOut<VersionId>>) -> Result<Answer, Declined> {
    match outs.into_iter().next() {
        Some(TrailOut::Save) => Ok(Answer::Save),
        Some(TrailOut::Restore(version)) => Ok(Answer::Restore(version)),
        Some(TrailOut::NothingToUndo) => Err(Declined::NothingToUndo),
        Some(TrailOut::NothingToRedo) => Err(Declined::NothingToRedo),
        Some(TrailOut::Busy) | None => Err(Declined::Busy),
    }
}

/// `shown` and the task of a save the trail allowed; a request the trail answered another way
/// starts nothing.
fn saves(
    shown: Shown,
    outs: Vec<TrailOut<VersionId>>,
    task: impl FnOnce() -> Task,
) -> (Shown, Carry) {
    match answered(outs) {
        Ok(Answer::Save) => (shown, Carry::Desktop(task())),
        Ok(Answer::Restore(_)) => declined(shown, Declined::Busy),
        Err(why) => declined(shown, why),
    }
}

/// `shown` and the task that puts back the version the trail named; a request the trail answered
/// another way starts nothing.
fn restores(
    shown: Shown,
    outs: Vec<TrailOut<VersionId>>,
    task: impl FnOnce(VersionId) -> Task,
) -> (Shown, Carry) {
    match answered(outs) {
        Ok(Answer::Restore(version)) => (shown, Carry::Desktop(task(version))),
        Ok(Answer::Save) => declined(shown, Declined::Busy),
        Err(why) => declined(shown, why),
    }
}

fn declined(shown: Shown, why: Declined) -> (Shown, Carry) {
    (shown, Carry::Declined(why))
}
