//! A plugin for the tests: it serves protocol v1 for files whose first line is its text, and
//! misbehaves in the one way its command line asks for. The wire faults are bayonet's fake plugin;
//! what this adds is the viewer's own messages and the three faults that need them.

mod serve;

fn main() -> std::process::ExitCode {
    serve::run()
}
