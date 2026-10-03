use super::*;
use crate::spawn::Spawn;
use crate::testing::RecordingSpawn;
use std::sync::Arc;

fn write(path: &std::path::Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn entry(name: &str, exec: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n{extra}")
}

fn png() -> Mime {
    Mime::parse("image/png").unwrap()
}

/// A scratch home with a viewer, an editor and a hidden entry that all declare `image/png`, and
/// a `mimeapps.list` that makes the editor the default and removes the viewer.
fn world() -> (tempfile::TempDir, DesktopApps, RecordingSpawn) {
    let scratch = tempfile::tempdir().unwrap();
    let mut env = Env::isolated(scratch.path());
    let spawn = RecordingSpawn::default();
    env.spawn = Arc::new(spawn.clone()) as Arc<dyn Spawn>;
    let apps = scratch.path().join("data/applications");
    write(
        &apps.join("viewer.desktop"),
        &entry("Viewer", "viewer %f", "MimeType=image/png;image/jpeg;\n"),
    );
    write(
        &apps.join("editor.desktop"),
        &entry("Editor", "editor --open %U", "MimeType=image/png;\n"),
    );
    write(
        &apps.join("hidden.desktop"),
        &entry(
            "Hidden",
            "hidden %f",
            "MimeType=image/png;\nNoDisplay=true\n",
        ),
    );
    write(
        &apps.join("zoom.desktop"),
        &entry("Zoom", "zoom %f", "MimeType=image/png;\n"),
    );
    write(
        &apps.join("other.desktop"),
        &entry("Other", "other %f", "MimeType=text/plain;\n"),
    );
    write(
        &scratch.path().join("config/mimeapps.list"),
        "[Default Applications]\nimage/png=editor.desktop;gone.desktop;\n\
         [Removed Associations]\nimage/png=viewer.desktop;\n",
    );
    let desktop = DesktopApps::new(env);
    (scratch, desktop, spawn)
}

#[test]
fn the_default_comes_first_and_removed_or_hidden_entries_are_left_out() {
    let (_scratch, apps, _spawn) = world();
    let offered: Vec<(String, String, Association)> = apps
        .apps_for(&png())
        .into_iter()
        .map(|app| (app.id.as_str().to_owned(), app.name, app.association))
        .collect();
    assert_eq!(
        offered,
        vec![
            (
                "editor.desktop".into(),
                "Editor".into(),
                Association::Default
            ),
            ("zoom.desktop".into(), "Zoom".into(), Association::Declared),
        ]
    );
}

#[test]
fn a_type_nobody_declares_has_no_apps() {
    let (_scratch, apps, _spawn) = world();
    assert!(apps.apps_for(&Mime::parse("video/mp4").unwrap()).is_empty());
}

#[test]
fn opening_with_an_app_starts_its_exec_line_on_the_file() {
    let (_scratch, apps, spawn) = world();
    let file = FilePath::new("/pics/a.png").unwrap();
    apps.open_with(&DesktopId::new("editor.desktop").unwrap(), &file)
        .unwrap();
    let started = spawn.started();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].program(), "editor");
    assert_eq!(started[0].args(), ["--open", "/pics/a.png"]);
}

#[test]
fn opening_with_an_app_that_is_not_installed_is_refused() {
    let (_scratch, apps, spawn) = world();
    let file = FilePath::new("/pics/a.png").unwrap();
    let error = apps
        .open_with(&DesktopId::new("gone.desktop").unwrap(), &file)
        .unwrap_err();
    assert!(matches!(error, PlatformError::Exec { .. }), "{error:?}");
    assert!(spawn.started().is_empty());
}
