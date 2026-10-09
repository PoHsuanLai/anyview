use super::*;
use anyview_ui::Look;
use ds::prelude::{Scheme, SystemPrefs, Theme};
use ds_blitz::TokioSpawner;
use ds_settings::{AppName, ConfigRoot};
use std::path::Path;
use std::time::Duration;

fn store(dir: &Path) -> Store {
    Store::new(ConfigRoot::Scratch(dir.to_path_buf()), AppName::QUIRE)
}

fn theme_of(look: &Look) -> Theme {
    look.appearance.theme
}

/// Replace the file the way the settings app does: a sibling written whole, then renamed over it.
fn replace(path: &Path, text: &str) {
    let sibling = path.with_extension("toml.new");
    std::fs::write(&sibling, text).unwrap();
    std::fs::rename(&sibling, path).unwrap();
}

fn dark_system() -> SystemPrefsSource {
    SystemPrefsSource::Fixed(SystemPrefs::default().with_scheme(Scheme::Dark))
}

#[test]
fn the_file_in_a_scratch_directory_is_followed_through_a_rename_over_it() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let path = store.path::<AppearanceFile>().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    replace(&path, "[appearance]\ntheme = \"light\"\n");

    let appearances = runtime.block_on(Appearances::follow(
        store,
        dark_system(),
        Arc::new(TokioSpawner::on(runtime.handle().clone())),
    ));
    assert_eq!(
        theme_of(&appearances.current()),
        Theme::Light,
        "the file's first read"
    );
    assert_eq!(
        appearances.current().system.scheme,
        Scheme::Dark,
        "the desktop's answer"
    );

    let mut feed = appearances.feed().0;
    replace(&path, "[appearance]\ntheme = \"dark\"\n");
    let arrived = runtime
        .block_on(async { tokio::time::timeout(Duration::from_secs(10), feed.changed()).await });
    assert!(
        matches!(arrived, Ok(Ok(()))),
        "the change arrives: {arrived:?}"
    );
    assert_eq!(theme_of(&feed.borrow()), Theme::Dark);
    assert_eq!(theme_of(&appearances.current()), Theme::Dark);
}

#[test]
fn a_desktop_with_no_file_gets_the_defaults_and_the_program_writes_none() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let path = store.path::<AppearanceFile>().unwrap();
    let appearances = runtime.block_on(Appearances::follow(
        store,
        SystemPrefsSource::Fixed(SystemPrefs::default()),
        Arc::new(TokioSpawner::on(runtime.handle().clone())),
    ));
    assert_eq!(theme_of(&appearances.current()), Theme::System);
    assert!(!path.exists(), "following never writes the appearance file");
    let mut files = Vec::new();
    let mut folders = vec![dir.path().to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
    }
    assert!(files.is_empty(), "nor any other file: {files:?}");
}

#[test]
fn the_look_carries_the_keys_the_file_set() {
    let mut environment = Environment::default();
    environment.settings.appearance.material_tint_alpha = ds_settings::Percent(64);
    let look = look_of(&environment);
    assert_eq!(look.tint_alpha, Some(environment.tint_alpha()));
    assert_eq!(look.stack, Some(environment.material_stack()));
    assert_eq!(
        look.appearance,
        environment.settings.appearance.appearance()
    );
}
