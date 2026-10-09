//! The integration tests of anyview, in one binary so they link once.

#![allow(clippy::unwrap_used)]

// The in-memory PDF builder of the pdf crate, compiled once for every file here that uses it.
mod audio_session;
mod media_hub;
mod media_plugins;
mod missing_helpers;
mod mpris_bus;
mod open_heic_raw;
mod open_image;
mod open_recording;
#[path = "../../../anyview-pdf/tests/pdf/support/mod.rs"]
mod pdf_fixture;
mod support;
mod window_fit;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "anyview",
        &["crash_report", "launch", "media_thread"],
        &[],
        include_str!("main.rs"),
    );
}
