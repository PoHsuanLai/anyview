//! Opening an address with `xdg-open`, the way mail sharing starts `xdg-email`: the desktop
//! chooses the program, and inside a sandbox `xdg-open` itself goes through the OpenURI portal.

use crate::env::Env;
use crate::error::PlatformError;
use crate::link::OpenLink;
use crate::spawn::Argv;

/// [`OpenLink`] through `xdg-open`.
#[derive(Debug, Clone)]
pub struct XdgOpen {
    env: Env,
}

impl XdgOpen {
    /// Links opened by starting programs through `env`.
    pub fn new(env: Env) -> Self {
        XdgOpen { env }
    }
}

impl OpenLink for XdgOpen {
    fn open(&self, uri: &str) -> Result<(), PlatformError> {
        // `--` so an address that begins with a dash is never read as an option.
        let argv = Argv::new("xdg-open", vec!["--".to_owned(), uri.to_owned()]);
        self.env.spawn.spawn(&argv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::RecordingSpawn;
    use std::sync::Arc;

    #[test]
    fn an_address_is_handed_to_xdg_open_after_a_double_dash() {
        let spawn = RecordingSpawn::default();
        let mut env = Env::isolated(std::path::Path::new("/nonexistent"));
        env.spawn = Arc::new(spawn.clone());
        XdgOpen::new(env).open("https://example.org/a b").unwrap();
        let started = spawn.started();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].program(), "xdg-open");
        assert_eq!(started[0].args(), ["--", "https://example.org/a b"]);
    }
}
