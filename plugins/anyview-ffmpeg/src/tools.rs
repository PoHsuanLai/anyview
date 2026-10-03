//! Finding the person's `ffmpeg` and `ffprobe`, and checking they are FFmpeg.

use crate::error::FfmpegError;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The oldest FFmpeg major this plugin drives: the options it passes (`-progress`, `-copypriorss`,
/// `-map 0:V`) all exist from 4.
const OLDEST_MAJOR: u32 = 4;

/// Where each program may be named, in the order they are tried.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookup {
    /// `--ffmpeg <absolute path>` from the manifest's arguments.
    pub ffmpeg_arg: Option<PathBuf>,
    /// `--ffprobe <absolute path>` from the manifest's arguments.
    pub ffprobe_arg: Option<PathBuf>,
    /// `ANYVIEW_FFMPEG`.
    pub ffmpeg_env: Option<OsString>,
    /// `ANYVIEW_FFPROBE`.
    pub ffprobe_env: Option<OsString>,
    /// `PATH`.
    pub path: Option<OsString>,
}

impl Lookup {
    /// The lookup the process's arguments and environment make.
    pub fn from_process(
        args: impl Iterator<Item = String>,
        ffmpeg_env: Option<OsString>,
        ffprobe_env: Option<OsString>,
        path: Option<OsString>,
    ) -> Lookup {
        let args: Vec<String> = args.collect();
        let after = |flag: &str| {
            args.iter()
                .position(|arg| arg == flag)
                .and_then(|at| args.get(at + 1))
                .map(PathBuf::from)
        };
        Lookup {
            ffmpeg_arg: after("--ffmpeg"),
            ffprobe_arg: after("--ffprobe"),
            ffmpeg_env,
            ffprobe_env,
            path,
        }
    }
}

/// The FFmpeg release a program reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// A release: `8.1`, `n7.1.1`, `6.0-ubuntu`.
    Release(u32),
    /// A build from the development tree (`N-123-gabc`), which has no major to compare.
    Development,
}

/// The two programs, found and checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    /// The `ffmpeg` program.
    pub ffmpeg: PathBuf,
    /// The `ffprobe` program.
    pub ffprobe: PathBuf,
    /// What `ffmpeg -version` said.
    pub version: Version,
}

impl Tools {
    /// Finds both programs and checks that they run and that FFmpeg is new enough.
    pub fn locate(lookup: &Lookup) -> Result<Tools, FfmpegError> {
        let ffmpeg = find(
            "ffmpeg",
            lookup.ffmpeg_arg.as_deref(),
            lookup.ffmpeg_env.as_deref(),
            lookup.path.as_deref(),
        )?;
        let ffprobe = find(
            "ffprobe",
            lookup.ffprobe_arg.as_deref(),
            lookup.ffprobe_env.as_deref(),
            lookup.path.as_deref(),
        )?;
        let version = version_of("ffmpeg", &ffmpeg)?;
        version_of("ffprobe", &ffprobe)?;
        Ok(Tools {
            ffmpeg,
            ffprobe,
            version,
        })
    }
}

/// The program `name`: the path an argument names, else the one the environment names, else the
/// first match on the search path. A path that is named and does not run is an error, never a
/// reason to look further, so a person who points at a program is not silently given another.
fn find(
    name: &'static str,
    arg: Option<&Path>,
    env: Option<&std::ffi::OsStr>,
    path: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, FfmpegError> {
    let named = arg
        .map(Path::to_path_buf)
        .or_else(|| env.filter(|value| !value.is_empty()).map(PathBuf::from));
    if let Some(candidate) = named {
        return if is_program(&candidate) {
            Ok(candidate)
        } else {
            Err(FfmpegError::ToolMissing {
                tool: name,
                looked: format!("{} is not an executable file", candidate.display()),
            })
        };
    }
    let found = path.and_then(|path| {
        std::env::split_paths(path)
            .map(|dir| dir.join(name))
            .find(|candidate| is_program(candidate))
    });
    found.ok_or_else(|| FfmpegError::ToolMissing {
        tool: name,
        looked: "not on the search path, and no --ffmpeg, --ffprobe, ANYVIEW_FFMPEG or ANYVIEW_FFPROBE names it"
            .to_owned(),
    })
}

fn is_program(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn version_of(tool: &'static str, program: &Path) -> Result<Version, FfmpegError> {
    let output = Command::new(program)
        .arg("-version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| FfmpegError::ToolBroken {
            tool,
            reason: format!("it did not run: {}", error.kind()),
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or_default();
    let version = parse_version(tool, first).ok_or_else(|| FfmpegError::ToolBroken {
        tool,
        reason: format!("it did not say \"{tool} version ...\": {first:?}"),
    })?;
    match version {
        Version::Release(major) if major < OLDEST_MAJOR => Err(FfmpegError::ToolBroken {
            tool,
            reason: format!("version {major} is older than the {OLDEST_MAJOR} this plugin needs"),
        }),
        Version::Release(_) | Version::Development => Ok(version),
    }
}

/// The version in the first line of `-version`: `ffmpeg version 8.1 Copyright ...`.
pub fn parse_version(tool: &str, line: &str) -> Option<Version> {
    let rest = line
        .strip_prefix(tool)?
        .trim_start()
        .strip_prefix("version")?;
    let word = rest.split_whitespace().next()?;
    let digits = word.strip_prefix('n').unwrap_or(word);
    if digits.starts_with("N-") {
        return Some(Version::Development);
    }
    let major: String = digits.chars().take_while(char::is_ascii_digit).collect();
    major.parse().ok().map(Version::Release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_the_first_line() {
        const CASES: &[(&str, &str, Option<Version>)] = &[
            (
                "release",
                "ffmpeg version 8.1 Copyright (c) 2000-2026",
                Some(Version::Release(8)),
            ),
            (
                "tagged",
                "ffmpeg version n7.1.1 Copyright",
                Some(Version::Release(7)),
            ),
            (
                "distro",
                "ffmpeg version 6.0-3ubuntu1 Copyright",
                Some(Version::Release(6)),
            ),
            (
                "development",
                "ffmpeg version N-117000-gabcdef Copyright",
                Some(Version::Development),
            ),
            ("other program", "mpv 0.40", None),
            ("no number", "ffmpeg version next", None),
        ];
        for (name, line, want) in CASES {
            assert_eq!(parse_version("ffmpeg", line), *want, "{name}");
        }
    }

    #[test]
    fn arguments_come_before_the_environment_and_a_named_path_is_never_skipped() {
        let lookup = Lookup::from_process(
            ["--ffmpeg".to_owned(), "/nonexistent/ffmpeg".to_owned()].into_iter(),
            Some("/usr/bin/ffmpeg".into()),
            None,
            Some("/usr/bin".into()),
        );
        assert_eq!(
            lookup.ffmpeg_arg,
            Some(PathBuf::from("/nonexistent/ffmpeg"))
        );
        let error = Tools::locate(&lookup).unwrap_err();
        assert!(
            matches!(error, FfmpegError::ToolMissing { tool: "ffmpeg", .. }),
            "{error}"
        );
    }

    #[test]
    fn the_environment_and_then_the_search_path_name_a_program() {
        let by_env = find(
            "sh",
            None,
            Some("/bin/sh".as_ref()),
            Some("/nowhere".as_ref()),
        );
        assert_eq!(by_env.unwrap(), PathBuf::from("/bin/sh"));
        let by_path = find(
            "sh",
            None,
            Some("".as_ref()),
            Some("/nowhere:/bin".as_ref()),
        );
        assert_eq!(by_path.unwrap(), PathBuf::from("/bin/sh"));
        assert!(find("sh", None, None, Some("/nowhere".as_ref())).is_err());
        assert!(find("sh", None, None, None).is_err());
    }
}
