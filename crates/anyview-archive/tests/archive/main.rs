//! The integration tests of anyview-archive, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod archive;
mod hostile_sevenz;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "archive",
        &["bounded_streams"],
        &[],
        include_str!("main.rs"),
    );
}
