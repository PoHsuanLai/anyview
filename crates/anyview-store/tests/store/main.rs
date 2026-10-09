//! The integration tests of anyview-store, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod concurrent_read;
mod history_file;
mod place;
mod save_in_place;
mod save_safety;
mod store_writer;
mod support;
mod version_store;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "store",
        &[],
        &[],
        include_str!("main.rs"),
    );
}
