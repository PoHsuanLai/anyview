//! The integration tests of anyview-plugin-fake, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod discovery;
mod export;
mod requests;
mod routing;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "plugin-fake",
        &[],
        &[],
        include_str!("main.rs"),
    );
}
