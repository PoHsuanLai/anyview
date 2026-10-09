//! `syntax-dump SYNTAXES_DIR OUT`: the default syntaxes and every `.sublime-syntax` in the folders under
//! `SYNTAXES_DIR` (one folder per source, in name order), packed into `OUT`.

use std::path::{Path, PathBuf};
use syntect::dumps::dump_to_uncompressed_file;
use syntect::parsing::SyntaxSet;

fn folders(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.retain(|path| path.is_dir());
    entries.sort();
    Ok(entries)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(dir), Some(out)) = (args.next(), args.next()) else {
        return Err("usage: syntax-dump SYNTAXES_DIR OUT".into());
    };
    // A folder at a time, so that one syntax can include another of its folder by file name.
    let folders = folders(Path::new(&dir))?;
    let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
    for folder in &folders {
        builder
            .add_from_folder(folder, true)
            .map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let set = builder.build();
    dump_to_uncompressed_file(&set, &out)?;
    println!(
        "{} syntaxes ({} folders) -> {out}",
        set.syntaxes().len(),
        folders.len()
    );
    Ok(())
}
