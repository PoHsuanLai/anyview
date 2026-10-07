//! Sharing by mail: `xdg-email` opens the person's mail program with the file attached. The
//! desktop has no share portal, so mail is the one target.

use crate::env::Env;
use crate::error::PlatformError;
use crate::share::{Share, ShareTarget};
use crate::spawn::Argv;
use anyview_core::FilePath;

/// [`Share`] to the mail program.
#[derive(Debug, Clone)]
pub struct MailShare {
    env: Env,
}

impl MailShare {
    /// Sharing that starts programs through `env`.
    pub fn new(env: Env) -> Self {
        MailShare { env }
    }
}

impl Share for MailShare {
    fn targets(&self) -> Vec<ShareTarget> {
        vec![ShareTarget::Mail]
    }

    async fn share(&self, file: &FilePath, target: ShareTarget) -> Result<(), PlatformError> {
        match target {
            ShareTarget::Mail => {
                let attachment = file.as_path().to_string_lossy().into_owned();
                let argv = Argv::new("xdg-email", vec!["--attach".to_owned(), attachment]);
                self.env.spawn.spawn(&argv)
            }
        }
    }
}
