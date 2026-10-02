//! Starting a program, the one effect Open With and mail sharing need that is not a bus call.

use crate::error::PlatformError;

/// A command line: a program and its arguments, never empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Argv {
    program: String,
    args: Vec<String>,
}

impl Argv {
    /// `program` with `args`.
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Argv {
        Argv {
            program: program.into(),
            args,
        }
    }

    /// `words` as a command, or `None` when there is no program in it.
    pub fn from_words(words: Vec<String>) -> Option<Argv> {
        let mut words = words.into_iter();
        let program = words.next().filter(|program| !program.is_empty())?;
        Some(Argv::new(program, words.collect()))
    }

    /// The program to run.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Its arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Starts a program and does not wait for it.
pub trait Spawn: std::fmt::Debug + Send + Sync {
    /// Start `argv` with no input and no output attached.
    fn spawn(&self, argv: &Argv) -> Result<(), PlatformError>;
}

/// The real thing: a child process of this one.
#[derive(Debug, Clone, Copy)]
pub struct ProcessSpawn;

impl Spawn for ProcessSpawn {
    fn spawn(&self, argv: &Argv) -> Result<(), PlatformError> {
        use std::process::{Command, Stdio};
        Command::new(argv.program())
            .args(argv.args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
            .map_err(|error| PlatformError::Spawn {
                program: argv.program().to_owned(),
                kind: error.kind(),
            })
    }
}

/// Starts nothing: what an isolated environment holds.
#[derive(Debug, Clone, Copy)]
pub struct RefuseSpawn;

impl Spawn for RefuseSpawn {
    fn spawn(&self, argv: &Argv) -> Result<(), PlatformError> {
        Err(PlatformError::Spawn {
            program: argv.program().to_owned(),
            kind: std::io::ErrorKind::PermissionDenied,
        })
    }
}
