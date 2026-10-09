//! The integration tests of anyview-export, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod pdf;
mod raster;
mod safety;
mod support;
mod text;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "export",
        &[],
        &["fixtures"],
        include_str!("main.rs"),
    );
}
