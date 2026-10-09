//! The integration tests of anyview-heif, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod requests;
mod routing;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "heif",
        &[],
        &[],
        include_str!("main.rs"),
    );
}
