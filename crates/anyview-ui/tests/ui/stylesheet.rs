//! The viewer's own CSS holds no literal design value: every colour, size, face and duration is a
//! quire token (`ds_lint`, the strictest profile).

use ds_lint::{LintConfig, Profile, assert_clean};

fn strict() -> LintConfig {
    LintConfig {
        profile: Profile::Strict,
        ..LintConfig::new(&ds::kits())
    }
}

#[test]
fn the_window_and_the_rendered_page_stylesheets_are_clean() {
    assert_clean(&anyview_ui::stylesheet(), &strict());
    assert_clean(include_str!("../../src/families/text/page.css"), &strict());
}
