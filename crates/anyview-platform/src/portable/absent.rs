//! The desktop abilities a platform without the desktop's services does not have, each behind its
//! trait: there is no mail target, no print dialog and no file chooser.
//! The host answers each with a "not available" outcome rather than an error.

use crate::error::PlatformError;
use crate::picker::{FileKinds, PickOutcome, Picker};
use crate::printer::{JobTitle, PrintOutcome, Printer};
use crate::share::{Share, ShareTarget};
use anyview_core::FilePath;

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
    fn present(&self) -> bool {
        false
    }

    async fn print(&self, _pdf: &[u8], _title: &JobTitle) -> Result<PrintOutcome, PlatformError> {
        Ok(PrintOutcome::NoDialog)
    }
}

/// [`Picker`] with no file dialog: every request answers [`PickOutcome::NoDialog`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPicker;

impl Picker for NoPicker {
    fn present(&self) -> bool {
        false
    }

    async fn pick(&self, _kinds: &FileKinds) -> Result<PickOutcome, PlatformError> {
        Ok(PickOutcome::NoDialog)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn each_absent_ability_says_so_and_does_nothing() {
        assert!(NoShare.targets().is_empty());
        let title = JobTitle("a".to_owned());
        assert_eq!(
            NoPrinter.print(b"%PDF", &title).await.unwrap(),
            PrintOutcome::NoDialog
        );
        assert_eq!(
            NoPicker.pick(&FileKinds::default()).await.unwrap(),
            PickOutcome::NoDialog
        );
        assert!(!NoPrinter.present() && !NoPicker.present());
    }
}
