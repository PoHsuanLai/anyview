//! Finding libheif's tools and making a picture with them.

use anyview_plugin_protocol::Capability;
use anyview_tool_kit::{
    Backend, Fit, Lookup, Picture, Stop, ToolError, find_tool, load_picture, run_tool, scaled,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The longest a decode of one file may take.
const DECODE_LIMIT: Duration = Duration::from_secs(120);
/// The longest a thumbnail may take.
const THUMBNAIL_LIMIT: Duration = Duration::from_secs(30);

/// The tools found on this machine.
#[derive(Debug)]
pub struct Heif {
    decoder: Option<PathBuf>,
    thumbnailer: Option<PathBuf>,
}

impl Heif {
    /// Finds `heif-dec` (else `heif-convert`) and `heif-thumbnailer`: `--heif-dec` and
    /// `--heif-thumbnailer` in the manifest's arguments, then `ANYVIEW_HEIF_DEC` and
    /// `ANYVIEW_HEIF_THUMBNAILER`, then the search path. A tool that is named and absent is
    /// logged and counts as not there.
    pub fn locate(lookup: &Lookup) -> Heif {
        let found = |flag, var, names: &[&str]| match find_tool(lookup, flag, var, names) {
            Ok(found) => found,
            Err(error) => {
                eprintln!("anyview-heif: {error}");
                None
            }
        };
        Heif {
            decoder: found(
                "--heif-dec",
                "ANYVIEW_HEIF_DEC",
                &["heif-dec", "heif-convert"],
            ),
            thumbnailer: found(
                "--heif-thumbnailer",
                "ANYVIEW_HEIF_THUMBNAILER",
                &["heif-thumbnailer"],
            ),
        }
    }

    fn missing() -> ToolError {
        ToolError::ToolMissing {
            tool: "heif-dec".to_owned(),
            looked: "neither heif-dec nor heif-convert is on the search path, and no --heif-dec or ANYVIEW_HEIF_DEC names one".to_owned(),
        }
    }
}

/// The PNG a tool wrote under `dir`: `out.png`, else the first of `out-1.png`, `out-2.png`…
fn written(dir: &Path) -> Result<PathBuf, ToolError> {
    let plain = dir.join("out.png");
    if plain.is_file() {
        return Ok(plain);
    }
    let mut numbered: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|error| ToolError::io("list the tool's output", &error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("out-") && name.ends_with(".png"))
        })
        .collect();
    numbered.sort_by_key(|path| {
        let digits: String = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        (digits.parse::<u64>().unwrap_or(u64::MAX), path.clone())
    });
    numbered
        .into_iter()
        .next()
        .ok_or_else(|| ToolError::Failed {
            tool: "heif-dec".to_owned(),
            message: "it wrote no picture".to_owned(),
        })
}

impl Backend for Heif {
    fn name(&self) -> &'static str {
        "heif"
    }

    fn provides(&self) -> Vec<Capability> {
        match (&self.decoder, &self.thumbnailer) {
            (Some(_), _) => vec![Capability::Thumbnail, Capability::Decode],
            (None, Some(_)) => vec![Capability::Thumbnail],
            (None, None) => Vec::new(),
        }
    }

    fn decode(&self, path: &Path, max_area: u64, stop: &Stop) -> Result<Picture, ToolError> {
        let decoder = self.decoder.as_ref().ok_or_else(Heif::missing)?;
        let dir =
            tempfile::tempdir().map_err(|error| ToolError::io("make a scratch folder", &error))?;
        let mut command = Command::new(decoder);
        command.arg(path).arg(dir.path().join("out.png"));
        run_tool(command, DECODE_LIMIT, stop)?;
        load_picture(&written(dir.path())?, Fit::Area(max_area))
    }

    fn thumbnail(&self, path: &Path, max_edge: u32, stop: &Stop) -> Result<Picture, ToolError> {
        if let Some(thumbnailer) = &self.thumbnailer {
            let dir = tempfile::tempdir()
                .map_err(|error| ToolError::io("make a scratch folder", &error))?;
            let out = dir.path().join("out.png");
            let mut command = Command::new(thumbnailer);
            command
                .args(["-s", &max_edge.to_string()])
                .arg(path)
                .arg(&out);
            match run_tool(command, THUMBNAIL_LIMIT, stop) {
                Ok(_) => return load_picture(&out, Fit::Edge(max_edge)),
                Err(error @ (ToolError::Cancelled | ToolError::TimedOut { .. })) => {
                    return Err(error);
                }
                // Any other failure: a decode may still do.
                Err(error) => eprintln!("anyview-heif: heif-thumbnailer: {error}"),
            }
        }
        // Developed whole and then scaled by the long edge, so the shape is exact.
        let picture = self.decode(path, u64::MAX, stop)?;
        Ok(scaled(picture, Fit::Edge(max_edge)))
    }
}
