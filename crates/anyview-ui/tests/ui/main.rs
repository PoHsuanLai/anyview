//! The integration tests of anyview-ui, in one binary so they link once.

#![allow(clippy::unwrap_used)]

// The in-memory PDF builder of the pdf crate, compiled once for every file here that uses it.
mod actions;
mod animation;
mod behaviour;
mod book_window;
mod changes;
mod context_menu;
mod context_menu_scale;
mod data_keys;
mod eased_scroll;
mod edits;
mod export_dialog;
mod failed_files;
mod file_formats;
mod fresh_view;
mod gone_files;
mod layers_and_races;
mod layout;
mod media_flows;
mod media_window;
mod missing_helpers;
mod opening_files;
mod pan_mode;
#[path = "../../../anyview-pdf/tests/pdf/support/mod.rs"]
mod pdf_fixture;
mod pdf_window;
mod platform_abilities;
mod registry;
mod relook;
mod stages;
mod stylesheet;
mod support;
mod table_stage;
mod text_stage;
mod tooltips;
mod viewer_window;

#[path = "../../../../dev/test_guard.rs"]
mod test_guard;

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    test_guard::check(
        env!("CARGO_MANIFEST_DIR"),
        "ui",
        &[],
        &["fixtures", "snapshots"],
        include_str!("main.rs"),
    );
}
