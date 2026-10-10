//! When where the person is in a file is worth a write. A window or a pane says where the person
//! is whenever a gesture settles, which is often; a write takes the store's lock and replaces a
//! file. So the first place of a file is written at once, the ones that follow within
//! [`REMEMBER_EVERY`] wait and replace each other, and the latest waiting one is written when the
//! wait ends or when the file is let go of. A place no different from the last is not written.
//!
//! The policy reads no clock: the caller says how long it has been since an origin of its own
//! (the same one each time) with every call, and does the writing and the waiting.

use anyview_core::{FilePath, FileStamp, Resume, Source};
use std::collections::HashMap;
use std::time::Duration;

/// The most often a file's place is written. A future `viewer.*` setting (FINDINGS).
pub const REMEMBER_EVERY: Duration = Duration::from_millis(500);

/// What a place that was noted asks of the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Noted {
    /// Nothing: it is where the last noted place was.
    Unchanged,
    /// Write this now.
    Write(Source, Resume),
    /// It waits, and nothing was waiting before: call [`Remembering::take`] for the file at `due`
    /// (a caller with no timer may let a later note or the file's [`Remembering::release`] do it).
    Wait {
        /// When the wait ends, on the caller's clock.
        due: Duration,
    },
    /// It waits in the place of an earlier one, whose wake is already arranged.
    Replaced,
}

/// What is known of one file's place.
#[derive(Debug, Clone)]
struct Entry {
    /// The file as it was when the last place was noted.
    stamp: FileStamp,
    /// The last place noted, written or waiting.
    last: Resume,
    /// When the last write was made.
    written: Duration,
    /// The latest place not yet written.
    waiting: Option<(Source, Resume)>,
}

/// The places of the files a caller shows, and which of them are to be written.
#[derive(Debug, Clone)]
pub struct Remembering {
    every: Duration,
    files: HashMap<FilePath, Entry>,
}

impl Remembering {
    /// A policy writing each file's place at most once in `every`.
    #[must_use]
    pub fn new(every: Duration) -> Remembering {
        Remembering {
            every,
            files: HashMap::new(),
        }
    }

    /// `resume` is where the person is in `source` at `now`.
    pub fn note(&mut self, source: Source, resume: Resume, now: Duration) -> Noted {
        let path = source.path().clone();
        let Some(entry) = self.files.get_mut(&path) else {
            self.files.insert(
                path,
                Entry {
                    stamp: source.stamp(),
                    last: resume.clone(),
                    written: now,
                    waiting: None,
                },
            );
            return Noted::Write(source, resume);
        };
        if entry.stamp == source.stamp() && entry.last == resume {
            return Noted::Unchanged;
        }
        entry.stamp = source.stamp();
        entry.last = resume.clone();
        let due = entry.written.saturating_add(self.every);
        if now >= due {
            entry.waiting = None;
            entry.written = now;
            return Noted::Write(source, resume);
        }
        let first = entry.waiting.is_none();
        entry.waiting = Some((source, resume));
        if first {
            Noted::Wait { due }
        } else {
            Noted::Replaced
        }
    }

    /// The place waiting for `path`, to be written now (`now` is when).
    pub fn take(&mut self, path: &FilePath, now: Duration) -> Option<(Source, Resume)> {
        let entry = self.files.get_mut(path)?;
        let waiting = entry.waiting.take()?;
        entry.written = now;
        Some(waiting)
    }

    /// The caller is done with `path` (another file replaced it, or the window closed): the place
    /// waiting for it, to be written now, and nothing remembered of the file.
    pub fn release(&mut self, path: &FilePath) -> Option<(Source, Resume)> {
        self.files.remove(path)?.waiting
    }

    /// Every place waiting, to be written now: the program is ending, or the pane is going away.
    pub fn take_all(&mut self, now: Duration) -> Vec<(Source, Resume)> {
        self.files
            .values_mut()
            .filter_map(|entry| {
                let waiting = entry.waiting.take()?;
                entry.written = now;
                Some(waiting)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, LineIndex, ModTime};

    const EVERY: Duration = Duration::from_millis(500);

    fn source(name: &str, len: u64) -> Source {
        Source::new(
            FilePath::new(format!("/files/{name}")).unwrap(),
            FileStamp {
                len: ByteLen(len),
                modified: ModTime(1),
            },
        )
    }

    fn line(n: u32) -> Resume {
        Resume::Text { line: LineIndex(n) }
    }

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// One call on the policy, and what it answers.
    enum Step {
        /// file, size of the file, line, at (ms), the answer
        Note(&'static str, u64, u32, u64, Noted),
        /// file, at (ms), the line taken (none: nothing waited)
        Take(&'static str, u64, Option<u32>),
        /// file, the line released (none: nothing waited)
        Release(&'static str, Option<u32>),
        /// at (ms), the file and line of every place taken
        TakeAll(u64, &'static [(&'static str, u32)]),
    }

    fn wrote(name: &str, len: u64, n: u32) -> Noted {
        Noted::Write(source(name, len), line(n))
    }

    #[test]
    fn places_are_written_at_most_once_in_an_interval_and_the_latest_is_never_lost() {
        use Step::*;
        let scenarios: Vec<(&str, Vec<Step>)> = vec![
            (
                "the first place is written at once; a burst after it is one wait, of the latest",
                vec![
                    Note("a", 1, 1, 0, wrote("a", 1, 1)),
                    Note("a", 1, 2, 10, Noted::Wait { due: ms(500) }),
                    Note("a", 1, 3, 20, Noted::Replaced),
                    Note("a", 1, 4, 30, Noted::Replaced),
                    Take("a", 500, Some(4)),
                    Take("a", 501, None),
                ],
            ),
            (
                "a place after the interval is written, with whatever was waiting dropped for it",
                vec![
                    Note("a", 1, 1, 0, wrote("a", 1, 1)),
                    Note("a", 1, 2, 100, Noted::Wait { due: ms(500) }),
                    Note("a", 1, 3, 600, wrote("a", 1, 3)),
                    Take("a", 700, None),
                    Note("a", 1, 4, 800, Noted::Wait { due: ms(1100) }),
                ],
            ),
            (
                "a place no different from the last gives no write, waiting or written",
                vec![
                    Note("a", 1, 1, 0, wrote("a", 1, 1)),
                    Note("a", 1, 1, 900, Noted::Unchanged),
                    Note("a", 1, 2, 910, wrote("a", 1, 2)),
                    Note("a", 1, 3, 920, Noted::Wait { due: ms(1410) }),
                    Note("a", 1, 3, 930, Noted::Unchanged),
                ],
            ),
            (
                "the same place in an edited file is a new place",
                vec![
                    Note("a", 1, 1, 0, wrote("a", 1, 1)),
                    Note("a", 2, 1, 1000, wrote("a", 2, 1)),
                ],
            ),
            (
                "files wait on their own, and letting one go gives its latest and forgets it",
                vec![
                    Note("a", 1, 1, 0, wrote("a", 1, 1)),
                    Note("b", 1, 1, 5, wrote("b", 1, 1)),
                    Note("a", 1, 2, 10, Noted::Wait { due: ms(500) }),
                    Note("b", 1, 2, 20, Noted::Wait { due: ms(505) }),
                    Release("a", Some(2)),
                    Release("a", None),
                    Note("a", 1, 2, 30, wrote("a", 1, 2)),
                    TakeAll(40, &[("b", 2)]),
                    TakeAll(41, &[]),
                ],
            ),
        ];
        for (name, steps) in scenarios {
            let mut policy = Remembering::new(EVERY);
            for (i, step) in steps.into_iter().enumerate() {
                let at = format!("{name}, step {i}");
                match step {
                    Note(file, len, n, now, want) => {
                        assert_eq!(
                            policy.note(source(file, len), line(n), ms(now)),
                            want,
                            "{at}"
                        );
                    }
                    Take(file, now, want) => {
                        let path = source(file, 1).path().clone();
                        let got = policy.take(&path, ms(now)).map(|(_, resume)| resume);
                        assert_eq!(got, want.map(line), "{at}");
                    }
                    Release(file, want) => {
                        let path = source(file, 1).path().clone();
                        let got = policy.release(&path).map(|(_, resume)| resume);
                        assert_eq!(got, want.map(line), "{at}");
                    }
                    TakeAll(now, want) => {
                        let mut got: Vec<(String, Resume)> = policy
                            .take_all(ms(now))
                            .into_iter()
                            .map(|(source, resume)| {
                                (source.path().as_path().display().to_string(), resume)
                            })
                            .collect();
                        got.sort_by(|a, b| a.0.cmp(&b.0));
                        let want: Vec<(String, Resume)> = want
                            .iter()
                            .map(|(file, n)| (format!("/files/{file}"), line(*n)))
                            .collect();
                        assert_eq!(got, want, "{at}");
                    }
                }
            }
        }
    }
}
