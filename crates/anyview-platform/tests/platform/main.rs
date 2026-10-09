//! The integration tests of anyview-platform, in one binary so they link once.

#![allow(clippy::unwrap_used)]

mod agent_intents;
mod desktop_services;
mod latchkey_instance;
mod media_session;
mod single_instance;
mod support;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "platform",
        &[],
        &[],
        include_str!("main.rs"),
    );
}
