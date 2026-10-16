//! Finding the person's `ffmpeg` and `ffprobe`, and checking they are FFmpeg.

use crate::error::FfmpegError;
use anyview_plugin_common::{Lookup, NotRunnable, find_program};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The oldest FFmpeg major this plugin drives: the options it passes (`-progress`, `-copypriorss`,
/// `-map 0:V`) all exist from 4.
const OLDEST_MAJOR: u32 = 4;

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
    /// Finds both programs and checks that ffmpeg runs and is new enough. ffprobe is only found: it
    /// is the same suite, and asking it for its version too would add a process start to every
    /// request.
    pub fn locate(lookup: &Lookup) -> Result<Tools, FfmpegError> {
        let ffmpeg = find(lookup, "ffmpeg")?;
        let ffprobe = find(lookup, "ffprobe")?;
        let version = version_of("ffmpeg", &ffmpeg)?;
        Ok(Tools {
            ffmpeg,
            ffprobe,
            version,
        })
    }
}

/// The program `name`: the path `--name` names, else the one `ANYVIEW_NAME` names, else the first
/// match on the search path. A path that is named and does not run is an error, never a reason to
/// look further (`find_program` is where that holds).
fn find(lookup: &Lookup, name: &'static str) -> Result<PathBuf, FfmpegError> {
    let flag = format!("--{name}");
    let var = format!("ANYVIEW_{}", name.to_ascii_uppercase());
    match find_program(lookup, &flag, &var, &[name]) {
        Ok(Some(program)) => Ok(program),
        Ok(None) => Err(FfmpegError::ToolMissing {
            tool: name,
            looked: "not on the search path, and no --ffmpeg, --ffprobe, ANYVIEW_FFMPEG or ANYVIEW_FFPROBE names it"
                .to_owned(),
        }),
        Err(NotRunnable(candidate)) => Err(FfmpegError::ToolMissing {
            tool: name,
            looked: format!("{} is not an executable file", candidate.display()),
        }),
    }
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

    fn lookup(args: &[&str], env: &[(&str, &str)], path: &str) -> Lookup {
        Lookup {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: env
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).into()))
                .collect(),
            path: Some(path.into()),
        }
    }

    #[test]
    fn a_named_program_that_is_missing_is_an_error_though_ffmpeg_is_on_the_path() {
        let lookup = lookup(
            &["--ffmpeg", "/nonexistent/ffmpeg"],
            &[("ANYVIEW_FFMPEG", "/usr/bin/ffmpeg")],
            "/usr/bin",
        );
        let error = Tools::locate(&lookup).unwrap_err();
        assert!(
            matches!(error, FfmpegError::ToolMissing { tool: "ffmpeg", .. }),
            "{error}"
        );
    }
}
