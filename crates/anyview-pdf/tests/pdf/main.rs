//! The integration tests of anyview-pdf, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod bind;
mod cancel;
mod edit;
mod export;
mod info;
mod outline_links;
mod search;
mod support;
mod tiles;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "pdf",
        &[],
        &[],
        include_str!("main.rs"),
    );
}
