//! Finding the person's own program: an argument of the manifest, then an environment variable,
//! then the search path.

use crate::error::ToolError;
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Where a plugin's programs may be named, in the order they are tried.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookup {
    /// The manifest's arguments.
    pub args: Vec<String>,
    /// The environment variables, as (name, value) pairs the plugin cares about.
    pub env: Vec<(String, OsString)>,
    /// `PATH`.
    pub path: Option<OsString>,
}

impl Lookup {
    /// The lookup of this process: its arguments and `PATH`, and the variables named in `vars`.
    pub fn from_process(vars: &[&str]) -> Lookup {
        Lookup {
            args: std::env::args().skip(1).collect(),
            env: vars
                .iter()
                .filter_map(|name| Some(((*name).to_owned(), std::env::var_os(name)?)))
                .collect(),
            path: std::env::var_os("PATH"),
        }
    }

    fn arg(&self, flag: &str) -> Option<PathBuf> {
        let at = self.args.iter().position(|arg| arg == flag)?;
        self.args.get(at + 1).map(PathBuf::from)
    }

    fn var(&self, name: &str) -> Option<&OsStr> {
        self.env
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_os_str())
            .filter(|value| !value.is_empty())
    }
}

/// The program that does one job. `flag` (`--heif-dec`) and `var` (`ANYVIEW_HEIF_DEC`) name it
/// outright, and then `names` are tried in order on the search path. A path that is named and does
/// not run is an error and never a reason to look further, so a person who points at a program is
/// not silently given another. `Ok(None)` when nothing is named and nothing is on the path.
pub fn find_tool(
    lookup: &Lookup,
    flag: &str,
    var: &str,
    names: &[&str],
) -> Result<Option<PathBuf>, ToolError> {
    let named = lookup
        .arg(flag)
        .or_else(|| lookup.var(var).map(PathBuf::from));
    if let Some(candidate) = named {
        return if is_program(&candidate) {
            Ok(Some(candidate))
        } else {
            Err(ToolError::ToolMissing {
                tool: names.first().copied().unwrap_or("tool").to_owned(),
                looked: format!("{} is not an executable file", candidate.display()),
            })
        };
    }
    let Some(path) = lookup.path.as_deref() else {
        return Ok(None);
    };
    Ok(names.iter().find_map(|name| {
        std::env::split_paths(path)
            .map(|dir| dir.join(name))
            .find(|candidate| is_program(candidate))
    }))
}

fn is_program(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(args: &[&str], env: &[(&str, &str)], path: &str) -> Lookup {
        Lookup {
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).into()))
                .collect(),
            path: Some(path.into()),
        }
    }

    #[test]
    fn an_argument_beats_the_environment_beats_the_path_and_a_named_path_is_never_skipped() {
        let found = |l: &Lookup| find_tool(l, "--sh", "T_SH", &["sh"]);
        assert_eq!(
            found(&lookup(
                &["--sh", "/bin/sh"],
                &[("T_SH", "/nowhere")],
                "/nowhere"
            ))
            .unwrap(),
            Some(PathBuf::from("/bin/sh"))
        );
        assert_eq!(
            found(&lookup(&[], &[("T_SH", "/bin/sh")], "/nowhere")).unwrap(),
            Some(PathBuf::from("/bin/sh"))
        );
        assert_eq!(
            found(&lookup(&[], &[("T_SH", "")], "/nowhere:/bin")).unwrap(),
            Some(PathBuf::from("/bin/sh"))
        );
        assert_eq!(found(&lookup(&[], &[], "/nowhere")).unwrap(), None);
        // Named and absent: an error, though sh is on the path.
        let error = found(&lookup(&["--sh", "/nowhere/sh"], &[], "/bin")).unwrap_err();
        assert!(matches!(error, ToolError::ToolMissing { .. }), "{error}");
    }

    #[test]
    fn the_first_name_that_is_on_the_path_wins() {
        let got = find_tool(
            &lookup(&[], &[], "/bin"),
            "--x",
            "T_X",
            &["no-such-tool", "sh"],
        );
        assert_eq!(got.unwrap(), Some(PathBuf::from("/bin/sh")));
    }
}
