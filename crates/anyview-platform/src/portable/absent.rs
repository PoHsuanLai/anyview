//! The desktop abilities a platform without the desktop's services does not have, each behind its
//! trait: Open With lists nothing, there is no mail target, no print dialog and no file chooser.
//! The host answers each with a "not available" outcome rather than an error.

use crate::apps::{AppEntry, AppsForType, DesktopId};
use crate::error::PlatformError;
use crate::picker::{PickOutcome, Picker};
use crate::printer::{JobTitle, PrintOutcome, Printer};
use crate::share::{Share, ShareTarget};
use anyview_core::{FilePath, Mime};

/// [`AppsForType`] with no application database: no application is offered.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoApps;

impl AppsForType for NoApps {
    fn apps_for(&self, _mime: &Mime) -> Vec<AppEntry> {
        Vec::new()
    }

    fn open_with(&self, app: &DesktopId, _file: &FilePath) -> Result<(), PlatformError> {
        // Nothing is ever offered, so nothing can be chosen: a request for one names an entry
        // that does not exist.
        Err(PlatformError::Exec {
            id: app.as_str().to_owned(),
            reason: "this platform lists no applications".to_owned(),
        })
    }
}

/// [`Share`] with nowhere to send a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoShare;

impl Share for NoShare {
    fn targets(&self) -> Vec<ShareTarget> {
        Vec::new()
    }

    async fn share(&self, _file: &FilePath, _target: ShareTarget) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// [`Printer`] with no print dialog: every request answers [`PrintOutcome::NoDialog`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPrinter;

impl Printer for NoPrinter {
    async fn print(&self, _pdf: &[u8], _title: &JobTitle) -> Result<PrintOutcome, PlatformError> {
        Ok(PrintOutcome::NoDialog)
    }
}

/// [`Picker`] with no file dialog: every request answers [`PickOutcome::NoDialog`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPicker;

impl Picker for NoPicker {
    async fn pick(&self) -> Result<PickOutcome, PlatformError> {
        Ok(PickOutcome::NoDialog)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn each_absent_ability_says_so_and_does_nothing() {
        let mime = Mime::parse("image/png").unwrap();
        assert!(NoApps.apps_for(&mime).is_empty());
        assert!(NoShare.targets().is_empty());
        let title = JobTitle("a".to_owned());
        assert_eq!(
            NoPrinter.print(b"%PDF", &title).await.unwrap(),
            PrintOutcome::NoDialog
        );
        assert_eq!(NoPicker.pick().await.unwrap(), PickOutcome::NoDialog);
    }
}
