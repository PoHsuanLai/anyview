//! Printing and revealing against a private session bus that has no portal and no file manager:
//! the answers a desktop without them gives.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::FilePath;
use anyview_platform::linux::{FileManagerReveal, PortalPrinter};
use anyview_platform::{JobTitle, PlatformError, PrintOutcome, Printer, Reveal};
use support::PrivateBus;

#[tokio::test]
async fn printing_without_a_portal_asks_the_caller_to_open_the_pdf_instead() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let outcome = PortalPrinter::new(bus.env())
        .print(b"%PDF-1.7", &JobTitle("Report".to_owned()))
        .await
        .unwrap();
    assert_eq!(outcome, PrintOutcome::NoDialog);
}

#[tokio::test]
async fn printing_without_a_bus_asks_the_caller_to_open_the_pdf_instead() {
    let scratch = tempfile::tempdir().unwrap();
    let env = anyview_platform::Env::isolated(scratch.path());
    let outcome = PortalPrinter::new(env)
        .print(b"%PDF-1.7", &JobTitle("Report".to_owned()))
        .await
        .unwrap();
    assert_eq!(outcome, PrintOutcome::NoDialog);
}

#[tokio::test]
async fn revealing_without_a_file_manager_reports_the_failed_call() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let file = FilePath::new("/pics/a.png").unwrap();
    let error = FileManagerReveal::new(bus.env())
        .reveal(&file)
        .await
        .unwrap_err();
    assert!(matches!(error, PlatformError::Bus { .. }), "{error:?}");
}
