//! The integration tests of anyview-image, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod breadth;
mod decode;
mod edit;
mod encode;
mod faithful_edit;
mod hostile;
mod peek;
mod picture_facts;
mod rotate;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "image",
        &[],
        &["fixtures"],
        include_str!("main.rs"),
    );
}
