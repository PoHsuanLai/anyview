//! The viewer: `anyview [FILE...]`. Everything the program does is in the library; this reads the
//! process's arguments, directory and environment once and hands them over.

use anyview::program::run;
use anyview_core::FilePath;
use anyview_platform::Env;
use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cwd = std::env::current_dir()
        .map_err(|error| error.to_string())
        .and_then(|dir| FilePath::new(dir).map_err(|error| error.to_string()));
    match cwd {
        Ok(cwd) => run(&args, &cwd, Env::from_process()),
        Err(error) => {
            eprintln!("anyview: cannot find the working directory: {error}");
            ExitCode::FAILURE
        }
    }
}
