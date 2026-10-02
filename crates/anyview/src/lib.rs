//! The viewer binary's parts: the command line, the runtime that owns every thread the program
//! runs, the seam that puts the views' work on it, the host that carries out what the windows
//! ask, the windows themselves and the program that joins them.

pub mod cli;
pub mod host;
pub mod program;
pub mod runtime;
pub mod seam;
pub mod window;
