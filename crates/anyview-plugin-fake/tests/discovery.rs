//! Finding plugins in scratch XDG directories.

// Test helpers sit outside `#[test]` functions, where the workspace denies `unwrap`; a test may unwrap.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_plugin::{Origin, PluginError};
use std::os::unix::fs::PermissionsExt;
use support::{FAKE, Scratch, Where, fake_manifest};

#[test]
fn manifests_are_found_in_the_user_and_the_system_directories() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "mine", 1, &[]);
    scratch.install(Where::System, "theirs", 1, &[]);
    let found = anyview_platform::discover(&scratch.env);
    let ids: Vec<_> = found
        .plugins
        .installed()
        .iter()
        .map(|plugin| (plugin.manifest.id.as_str().to_owned(), plugin.origin))
        .collect();
    assert_eq!(
        ids,
        [
            ("mine".to_owned(), Origin::User),
            ("theirs".to_owned(), Origin::System)
        ]
    );
    assert!(found.rejected.is_empty());
}

#[test]
fn the_user_directory_overrides_the_system_directory() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "tool", 1, &["--fault", "mute"]);
    scratch.install(Where::User, "tool", 1, &[]);
    let found = anyview_platform::discover(&scratch.env);
    assert_eq!(found.plugins.installed().len(), 1);
    let plugin = &found.plugins.installed()[0];
    assert_eq!(plugin.origin, Origin::User);
    assert!(plugin.manifest.program.as_ref().unwrap().args.is_empty());
}

#[test]
fn a_plugin_with_the_same_id_and_a_newer_protocol_than_the_viewer_is_set_aside() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "tool", 1, &[]);
    scratch.install(Where::User, "tool", 2, &[]);
    let found = anyview_platform::discover(&scratch.env);
    assert_eq!(found.plugins.installed()[0].manifest.protocol, 1);
    assert_eq!(
        found.plugins.unusable()[0].reason,
        PluginError::ProtocolUnsupported {
            protocol: 2,
            supported: 1
        }
    );
}

#[test]
fn malformed_and_misnamed_manifests_are_rejected_without_hiding_the_good_ones() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "good", 1, &[]);
    let bad = scratch.write_manifest(Where::User, "bad.toml", "id = \n");
    let misnamed = scratch.write_manifest(
        Where::User,
        "other.toml",
        &fake_manifest("not-other", 1, &[]),
    );
    scratch.write_manifest(Where::User, "notes.txt", "not a manifest, not read");
    let found = anyview_platform::discover(&scratch.env);
    assert_eq!(found.plugins.installed().len(), 1);
    let rejected: Vec<_> = found
        .rejected
        .iter()
        .map(|r| (r.file.clone(), format!("{:?}", r.error)))
        .collect();
    assert_eq!(rejected.len(), 2);
    assert_eq!(rejected[0].0, bad);
    assert!(rejected[0].1.starts_with("Syntax"), "{}", rejected[0].1);
    assert_eq!(rejected[1].0, misnamed);
    assert!(
        rejected[1].1.starts_with("IdFileMismatch"),
        "{}",
        rejected[1].1
    );
}

#[test]
fn a_missing_or_unrunnable_program_makes_the_plugin_unusable() {
    let scratch = Scratch::new();
    let missing = fake_manifest("gone", 1, &[]).replace(FAKE, "/nonexistent/anyview-gone");
    scratch.write_manifest(Where::User, "gone.toml", &missing);
    let plain = scratch.dir.path().join("plain-file");
    std::fs::write(&plain, "not a program").unwrap();
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
    let flat = fake_manifest("flat", 1, &[]).replace(FAKE, plain.to_str().unwrap());
    scratch.write_manifest(Where::User, "flat.toml", &flat);
    let found = anyview_platform::discover(&scratch.env);
    assert!(found.plugins.installed().is_empty());
    let reasons: Vec<_> = found
        .plugins
        .unusable()
        .iter()
        .map(|u| (u.id.as_str().to_owned(), u.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        [
            (
                "flat".to_owned(),
                PluginError::NotExecutable { path: plain }
            ),
            (
                "gone".to_owned(),
                PluginError::FileMissing {
                    path: "/nonexistent/anyview-gone".into()
                }
            )
        ]
    );
}

#[test]
fn a_broken_user_copy_does_not_hide_the_working_system_one() {
    let scratch = Scratch::new();
    scratch.install(Where::System, "tool", 1, &[]);
    let broken = fake_manifest("tool", 1, &[]).replace(FAKE, "/nonexistent/tool");
    scratch.write_manifest(Where::User, "tool.toml", &broken);
    let found = anyview_platform::discover(&scratch.env);
    assert_eq!(found.plugins.installed()[0].origin, Origin::System);
}

#[test]
fn no_plugin_folders_is_an_empty_registry() {
    let scratch = Scratch::new();
    let found = anyview_platform::discover(&scratch.env);
    assert!(found.plugins.installed().is_empty());
    assert!(found.rejected.is_empty());
}
