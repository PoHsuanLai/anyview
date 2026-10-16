//! Finding the person's own program (the search itself is `anyview-plugin-common`'s), with the
//! error these plugins raise when it is not there.

use crate::error::ToolError;
use anyview_plugin_common::{Lookup, find_program};
use std::path::PathBuf;

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
    find_program(lookup, flag, var, names).map_err(|not_runnable| ToolError::ToolMissing {
        tool: names.first().copied().unwrap_or("tool").to_owned(),
        looked: format!("{} is not an executable file", not_runnable.0.display()),
    })
}
