//! The integration tests of anyview-text, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod hostile_sheets;
mod peek;
mod sheets;
mod support;
mod windowing;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "text",
        &["peek_memory"],
        &["fixtures"],
        include_str!("main.rs"),
    );
}
