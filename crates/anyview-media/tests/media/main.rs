//! The integration tests of anyview-media, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod builtin;
mod driver;
mod session;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "media",
        &[],
        &["fixtures"],
        include_str!("main.rs"),
    );
}
