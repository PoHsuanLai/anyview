//! `anyview [FILE|URI...]`, `anyview --peek FILE`, `anyview --play FILE`.

use anyview_core::{CoreError, FilePath};
use anyview_platform::Request;
use percent_encoding::percent_decode_str;
use std::ffi::OsString;

/// How the program is used, for `--help` and for a usage error.
pub const USAGE: &str = "usage: anyview [FILE...]\n       anyview --peek FILE\n       anyview --play FILE\n       anyview --help\n       anyview --version";

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Say how the program is used and stop.
    Help,
    /// Say which version this is and stop.
    Version,
    /// Do what a launch does: open files, peek at one or play one. No file at all is what the bus
    /// activation starts the program with: it serves until a request arrives.
    Launch(Request),
}

/// Why the arguments are not an invocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    /// A flag the program does not have.
    #[error("unknown option {0:?}")]
    UnknownFlag(String),
    /// A flag that takes a file was given none.
    #[error("{0} needs a file")]
    MissingFile(&'static str),
    /// A flag that takes one file was given several.
    #[error("{0} takes one file")]
    ManyFiles(&'static str),
    /// `--peek` and `--play` were both given, or one of them with another flag's meaning.
    #[error("--peek and --play cannot be combined")]
    Conflict,
    /// An argument that names no file.
    #[error("an empty file name")]
    EmptyName,
    /// A file name that is not valid UTF-8, which the viewer's history cannot hold.
    #[error("{0:?} is not valid UTF-8")]
    NotUtf8(OsString),
    /// A URI that is not a file on this machine (`%U` hands the viewer whatever the file manager has).
    #[error("{0:?} is not a local file")]
    NotLocal(String),
    /// The working directory is not an absolute path (a broken environment).
    #[error(transparent)]
    Path(#[from] CoreError),
}

/// What a flag does to the files that follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Open,
    Peek,
    Play,
}

impl Mode {
    fn flag(self) -> &'static str {
        match self {
            Mode::Open => "FILE",
            Mode::Peek => "--peek",
            Mode::Play => "--play",
        }
    }
}

/// `args` (without the program name) as an invocation, a relative file named against `cwd`.
pub fn parse(args: &[OsString], cwd: &FilePath) -> Result<Invocation, CliError> {
    let mut mode = Mode::Open;
    let mut files = Vec::new();
    let mut flags_over = false;
    for arg in args {
        let text = arg.to_str().ok_or_else(|| CliError::NotUtf8(arg.clone()))?;
        if flags_over || !text.starts_with('-') || text == "-" {
            files.push(resolve(text, cwd)?);
            continue;
        }
        match text {
            "--" => flags_over = true,
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "--peek" => mode = chosen(mode, Mode::Peek)?,
            "--play" => mode = chosen(mode, Mode::Play)?,
            other => return Err(CliError::UnknownFlag(other.to_owned())),
        }
    }
    let request = match mode {
        Mode::Open => Request::Open(files),
        Mode::Peek => Request::Peek(one(mode, files)?),
        Mode::Play => Request::Play(one(mode, files)?),
    };
    Ok(Invocation::Launch(request))
}

/// `next` as the mode, unless another flag already took it.
fn chosen(now: Mode, next: Mode) -> Result<Mode, CliError> {
    match now {
        Mode::Open => Ok(next),
        Mode::Peek | Mode::Play => Err(CliError::Conflict),
    }
}

/// The one file a flag names.
fn one(mode: Mode, mut files: Vec<FilePath>) -> Result<FilePath, CliError> {
    match (files.pop(), files.is_empty()) {
        (Some(file), true) => Ok(file),
        (None, _) => Err(CliError::MissingFile(mode.flag())),
        (Some(_), false) => Err(CliError::ManyFiles(mode.flag())),
    }
}

/// The path a `file://` URI names (a host of `localhost` or none), percent-decoded; `None` for a
/// plain path, an error for any other scheme.
fn uri_path(text: &str) -> Result<Option<String>, CliError> {
    let Some((scheme, rest)) = text.split_once("://") else {
        return Ok(None);
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    {
        return Ok(None);
    }
    let local = scheme
        .eq_ignore_ascii_case("file")
        .then(|| rest.strip_prefix("localhost").unwrap_or(rest))
        .filter(|path| path.starts_with('/'));
    let Some(path) = local else {
        return Err(CliError::NotLocal(text.to_owned()));
    };
    percent_decode_str(path)
        .decode_utf8()
        .map(|decoded| Some(decoded.into_owned()))
        .map_err(|_| CliError::NotUtf8(OsString::from(text)))
}

fn resolve(text: &str, cwd: &FilePath) -> Result<FilePath, CliError> {
    if text.is_empty() {
        return Err(CliError::EmptyName);
    }
    let decoded = uri_path(text)?;
    let text = decoded.as_deref().unwrap_or(text);
    Ok(FilePath::new(cwd.as_path().join(text))?)
}
