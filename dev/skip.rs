//! What a test that needs the real player does when this machine lacks what it needs (an mpv, its C
//! plugin, a graphics adapter, a private bus). Included by the integration binaries that play, with
//! `#[path = "../../../../dev/skip.rs"] mod skip;` beside `test_guard`.
#![allow(dead_code)]

use std::path::PathBuf;

/// The variable CI's `player` job sets: with it, a missing piece fails the test instead of skipping it.
const REQUIRE: &str = "ANYVIEW_REQUIRE_PLAYER";

/// Say a test is skipped because `why`, or, where `ANYVIEW_REQUIRE_PLAYER` is set, fail it: a job
/// that exists to run the player must not pass by running nothing. The caller returns `None` after.
pub fn skipped(why: &str) {
    assert!(
        std::env::var_os(REQUIRE).is_none(),
        "{why}; {REQUIRE} is set, so this test may not skip"
    );
    eprintln!("SKIPPED: {why}");
}

/// The program `name` on the search path, when there is one.
pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The programs that play, when this machine has them: the stock `mpv` (`MPV_WGPU_MPV`, else the
/// first on the search path) and the C plugin built from mpv-wgpu (`MPV_WGPU_CPLUGIN`). `None`, with
/// a note (or a failure, where the player is required), when either is missing.
pub fn mpv_programs() -> Option<(PathBuf, PathBuf)> {
    let cplugin = std::env::var_os("MPV_WGPU_CPLUGIN").map(PathBuf::from);
    let mpv = std::env::var_os("MPV_WGPU_MPV")
        .map(PathBuf::from)
        .or_else(|| on_path("mpv"));
    match (mpv, cplugin) {
        (Some(mpv), Some(cplugin)) if cplugin.is_file() => Some((mpv, cplugin)),
        (None, _) => {
            skipped("no mpv (set MPV_WGPU_MPV or put mpv on the search path)");
            None
        }
        (Some(_), _) => {
            skipped("no mpv-wgpu C plugin (build mpv-wgpu-cplugin, set MPV_WGPU_CPLUGIN)");
            None
        }
    }
}
