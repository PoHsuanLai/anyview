//! The integration tests of anyview-peek, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod book;
mod frames;
mod injected;
mod isolation;
mod location_privacy;
mod media;
mod natural;
mod no_media;
mod office;
mod pane;
mod parts;
mod peek;
mod picture_gpu;
mod probe;
mod support;
mod worker;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "peek",
        &[],
        &["fixtures", "snapshots"],
        include_str!("main.rs"),
    );
}
