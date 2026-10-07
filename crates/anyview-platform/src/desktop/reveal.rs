//! Showing a file in the file manager through `org.freedesktop.FileManager1.ShowItems`.

use crate::env::Env;
use crate::error::PlatformError;
use crate::reveal::Reveal;
use crate::uri::file_uri;
use anyview_core::FilePath;

const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER_PATH: &str = "/org/freedesktop/FileManager1";
/// No startup notification id: the window is not launched from a click the manager could
/// attribute.
const NO_STARTUP_ID: &str = "";

/// [`Reveal`] through the file manager's own bus name.
#[derive(Debug, Clone)]
pub struct FileManagerReveal {
    env: Env,
}

impl FileManagerReveal {
    /// A reveal on `env`'s session bus.
    pub fn new(env: Env) -> Self {
        FileManagerReveal { env }
    }
}

impl Reveal for FileManagerReveal {
    async fn reveal(&self, file: &FilePath) -> Result<(), PlatformError> {
        let connection = self
            .env
            .session_builder()?
            .build()
            .await
            .map_err(|error| PlatformError::bus("connect to the session bus", error))?;
        connection
            .call_method(
                Some(FILE_MANAGER),
                FILE_MANAGER_PATH,
                Some(FILE_MANAGER),
                "ShowItems",
                &(vec![file_uri(file)], NO_STARTUP_ID),
            )
            .await
            .map(drop)
            .map_err(|error| PlatformError::bus("show the file in the file manager", error))
    }
}
