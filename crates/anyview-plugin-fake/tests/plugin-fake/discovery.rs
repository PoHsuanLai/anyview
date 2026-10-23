//! Finding plugins in scratch XDG directories: Extensions under `quire/extensions`, and for one
//! more release the flat manifests under `anyview/plugins`.

use crate::support;

use anyview_plugin::{Origin, PluginError, Route};
use anyview_plugin_protocol::Capability;
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
fn a_named_folder_stands_in_for_the_users_directory() {
    let scratch = Scratch::new();
    scratch.install(Where::User, "mine", 1, &[]);
    scratch.install(Where::System, "theirs", 1, &[]);
    let found = anyview_platform::discover_in(&scratch.env, &scratch.env.dirs.config);
    let ids: Vec<_> = found
        .plugins
        .installed()
        .iter()
        .map(|plugin| plugin.manifest.id.as_str().to_owned())
        .collect();
    assert_eq!(ids, ["theirs"]);
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
    let bad = scratch.write_extension(Where::User, "bad", "id = \n");
    let misnamed =
        scratch.write_extension(Where::User, "other", &fake_manifest("not-other", 1, &[]));
    scratch.write_legacy(Where::User, "notes.txt", "not a manifest, not read");
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
        rejected[1].1.starts_with("IdFolderMismatch"),
        "{}",
        rejected[1].1
    );
}

#[test]
fn a_missing_or_unrunnable_program_makes_the_plugin_unusable() {
    let scratch = Scratch::new();
    let missing = fake_manifest("gone", 1, &[]).replace(FAKE, "/nonexistent/anyview-gone");
    scratch.write_extension(Where::User, "gone", &missing);
    let plain = scratch.dir.path().join("plain-file");
    std::fs::write(&plain, "not a program").unwrap();
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
    let flat = fake_manifest("flat", 1, &[]).replace(FAKE, plain.to_str().unwrap());
    scratch.write_extension(Where::User, "flat", &flat);
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
    scratch.write_extension(Where::User, "tool", &broken);
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

const HEIF: &str = "org.quire.anyview.Heif";

/// A case: its name, what is installed (an id, and whether it is in the old place), and the program
/// arguments of the copy that serves.
type Case = (
    &'static str,
    &'static [(&'static str, bool)],
    &'static [&'static str],
);

#[test]
fn a_shipped_plugin_in_the_old_place_is_the_same_plugin_and_the_new_place_wins() {
    // name, what is installed (old id, or the new one), the ids that serve, and the program
    // arguments of the copy that serves: the old copy is given an argument to tell it apart
    let cases: [Case; 3] = [
        ("old place only", &[("heif", true)], &["--old"]),
        ("new place only", &[(HEIF, false)], &[]),
        ("both at one root", &[("heif", true), (HEIF, false)], &[]),
    ];
    for (name, installed, args) in cases {
        let scratch = Scratch::new();
        for (id, old) in installed {
            if *old {
                scratch.install_legacy(Where::User, id, 1, &["--old"]);
            } else {
                scratch.install(Where::User, id, 1, &[]);
            }
        }
        let found = anyview_platform::discover(&scratch.env);
        let serving = found.plugins.installed();
        assert_eq!(serving.len(), 1, "{name}");
        assert_eq!(serving[0].manifest.id.as_str(), HEIF, "{name}");
        assert_eq!(
            serving[0].manifest.program.as_ref().unwrap().args,
            args,
            "{name}"
        );
    }
}

#[test]
fn a_turned_off_plugin_serves_nothing_and_is_off_not_missing() {
    // name, where it is installed, the id Settings turned off
    let cases = [
        ("an Extension", "fake", "fake"),
        ("a shipped plugin still in the old place", "heif", HEIF),
    ];
    for (name, installed, off) in cases {
        let scratch = Scratch::new();
        if installed == "heif" {
            scratch.install_legacy(Where::System, installed, 1, &[]);
        } else {
            scratch.install(Where::System, installed, 1, &[]);
        }
        scratch.turn_off(&[off]);
        let plugins = anyview_platform::discover(&scratch.env).plugins;
        assert!(plugins.installed().is_empty(), "{name}");
        match plugins.route(Capability::Probe, &support::book()) {
            Route::Off(plugin) => {
                assert_eq!(plugin.id(), off, "{name}");
                assert_eq!(plugin.name(), format!("Fake {installed}"), "{name}");
            }
            Route::Served(_) | Route::Missing(_) | Route::Unserved => panic!("{name}: off"),
        }
    }
}
