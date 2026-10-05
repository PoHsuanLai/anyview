//! Finding LibRaw's (or dcraw's) program and making a picture with it.

use anyview_plugin_protocol::Capability;
use anyview_tool_kit::{
    Backend, Fit, Lookup, Picture, Stop, ToolError, find_tool, load_picture, run_tool, scaled,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The longest a development of one file may take.
const DECODE_LIMIT: Duration = Duration::from_secs(180);
/// The longest the extraction of a preview may take.
const PREVIEW_LIMIT: Duration = Duration::from_secs(30);

/// Which program, and so how it is run.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Program {
    /// LibRaw's sample program: writes files beside its input.
    DcrawEmu(PathBuf),
    /// dcraw: writes to standard output with `-c`.
    Dcraw(PathBuf),
}

/// The program found on this machine.
#[derive(Debug)]
pub struct Raw {
    program: Option<Program>,
}

impl Raw {
    /// Finds `dcraw_emu` (`--dcraw-emu`, `ANYVIEW_DCRAW_EMU`, the search path), else `dcraw`
    /// (`--dcraw`, `ANYVIEW_DCRAW`, the search path). A program that is named and absent is logged
    /// and counts as not there.
    pub fn locate(lookup: &Lookup) -> Raw {
        let found = |flag, var, name: &str| match find_tool(lookup, flag, var, &[name]) {
            Ok(found) => found,
            Err(error) => {
                eprintln!("anyview-raw: {error}");
                None
            }
        };
        let program = found("--dcraw-emu", "ANYVIEW_DCRAW_EMU", "dcraw_emu")
            .map(Program::DcrawEmu)
            .or_else(|| found("--dcraw", "ANYVIEW_DCRAW", "dcraw").map(Program::Dcraw));
        Raw { program }
    }

    fn program(&self) -> Result<&Program, ToolError> {
        self.program.as_ref().ok_or_else(|| ToolError::ToolMissing {
            tool: "dcraw_emu".to_owned(),
            looked: "neither dcraw_emu nor dcraw is on the search path, and no --dcraw-emu, --dcraw, ANYVIEW_DCRAW_EMU or ANYVIEW_DCRAW names one".to_owned(),
        })
    }

    /// Runs the program on `path` with `args`, in a scratch folder, and returns the folder and the
    /// picture file the program made in it.
    fn make(
        &self,
        path: &Path,
        args: &[&str],
        limit: Duration,
        stop: &Stop,
    ) -> Result<(tempfile::TempDir, PathBuf), ToolError> {
        let program = self.program()?;
        let dir =
            tempfile::tempdir().map_err(|error| ToolError::io("make a scratch folder", &error))?;
        // A link under a plain name: the tool names its output after its input and must not
        // write into the person's folder.
        let extension: String = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("raw")
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        let link = dir.path().join(format!("in.{extension}"));
        std::os::unix::fs::symlink(path, &link)
            .map_err(|error| ToolError::io("link the file", &error))?;
        let out = match program {
            Program::DcrawEmu(tool) => {
                let mut command = Command::new(tool);
                command.args(args).arg(&link);
                run_tool(command, limit, stop)?;
                produced(dir.path(), &link)?
            }
            Program::Dcraw(tool) => {
                let mut command = Command::new(tool);
                command.arg("-c").args(args).arg(&link);
                let ran = run_tool(command, limit, stop)?;
                let out = dir.path().join("out.pnm");
                std::fs::write(&out, ran.stdout)
                    .map_err(|error| ToolError::io("keep the tool's output", &error))?;
                out
            }
        };
        Ok((dir, out))
    }
}

/// The one file `dcraw_emu` wrote next to `link`.
fn produced(dir: &Path, link: &Path) -> Result<PathBuf, ToolError> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|error| ToolError::io("list the tool's output", &error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path != link)
        .collect();
    files.sort();
    files.into_iter().next().ok_or_else(|| ToolError::Failed {
        tool: "dcraw_emu".to_owned(),
        message: "it wrote no picture".to_owned(),
    })
}

impl Backend for Raw {
    fn name(&self) -> &'static str {
        "raw"
    }

    fn provides(&self) -> Vec<Capability> {
        if self.program.is_some() {
            vec![Capability::Thumbnail, Capability::Decode]
        } else {
            Vec::new()
        }
    }

    fn decode(&self, path: &Path, max_area: u64, stop: &Stop) -> Result<Picture, ToolError> {
        let (_dir, out) = self.make(path, &["-w"], DECODE_LIMIT, stop)?;
        load_picture(&out, Fit::Area(max_area))
    }

    fn thumbnail(&self, path: &Path, max_edge: u32, stop: &Stop) -> Result<Picture, ToolError> {
        match self.make(path, &["-e"], PREVIEW_LIMIT, stop) {
            Ok((_dir, out)) => return load_picture(&out, Fit::Edge(max_edge)),
            Err(error @ (ToolError::Cancelled | ToolError::TimedOut { .. })) => return Err(error),
            // No preview in the file, or one the tool cannot extract: develop it small instead.
            Err(error) => eprintln!("anyview-raw: no preview: {error}"),
        }
        // Developed whole and then scaled by the long edge, so the shape is exact.
        let picture = self.decode(path, u64::MAX, stop)?;
        Ok(scaled(picture, Fit::Edge(max_edge)))
    }
}
