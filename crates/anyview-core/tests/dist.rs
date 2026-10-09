//! The packaging in `dist/` against the vocabulary: the desktop entry claims exactly the media
//! types the viewer opens, and the install and uninstall scripts do what they say in a staging
//! directory, never on the real system.

use anyview_core::claimed_mimes;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn entry() -> String {
    std::fs::read_to_string(repo().join("dist/org.quire.Anyview.desktop"))
        .expect("dist/org.quire.Anyview.desktop is readable")
}

fn line<'a>(entry: &'a str, key: &str) -> &'a str {
    entry
        .lines()
        .find_map(|l| l.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
        .unwrap_or_else(|| panic!("the desktop entry has no {key}"))
}

/// The `MimeType` value the map yields: sorted types, each followed by a semicolon.
fn expected_mime_line() -> String {
    claimed_mimes()
        .iter()
        .map(|mime| format!("{};", mime.as_str()))
        .collect()
}

#[test]
fn the_desktop_entry_claims_what_the_viewer_opens() {
    let entry = entry();
    assert_eq!(
        line(&entry, "MimeType"),
        expected_mime_line(),
        "regenerate the line: cargo test -p anyview-core --test dist print_mime_line -- --ignored --nocapture"
    );
}

/// The registry's canonical types and aliases on this machine, when shared-mime-info is installed
/// (a test never needs it: a machine without it skips the check).
fn registry() -> Option<(Vec<String>, Vec<String>)> {
    let read = |name: &str| std::fs::read_to_string(Path::new("/usr/share/mime").join(name)).ok();
    let types = read("types")?;
    let aliases = read("aliases")?;
    Some((
        types.split_whitespace().map(str::to_owned).collect(),
        aliases
            .lines()
            .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
            .collect(),
    ))
}

#[test]
fn every_type_the_desktop_entry_claims_is_a_canonical_type_of_the_registry() {
    let Some((types, aliases)) = registry() else {
        return;
    };
    for mime in claimed_mimes() {
        let text = mime.as_str();
        assert!(
            !aliases.iter().any(|alias| alias == text),
            "{text} is an alias: the registry names it by its canonical type"
        );
        assert!(
            types.iter().any(|known| known == text),
            "{text} is not in the registry"
        );
    }
}

#[test]
fn the_desktop_entry_names_the_app_the_window_carries() {
    let entry = entry();
    assert_eq!(line(&entry, "Name"), "Viewer");
    assert_eq!(line(&entry, "GenericName"), "File Viewer");
    assert_eq!(line(&entry, "Exec"), "anyview %U");
    assert_eq!(line(&entry, "Icon"), "org.quire.Anyview");
    assert_eq!(line(&entry, "StartupWMClass"), "org.quire.Anyview");
    // The bus name is `org.quire.Anyview1` with its own interface, not org.freedesktop.Application.
    assert_eq!(line(&entry, "DBusActivatable"), "false");
}

/// Prints the line to paste into the desktop entry.
#[test]
#[ignore = "prints the MimeType line; run it to regenerate the entry"]
fn print_mime_line() {
    println!("MimeType={}", expected_mime_line());
}

#[test]
fn install_and_uninstall_stage_and_remove_exactly_their_files() {
    let output = Command::new("bash")
        .arg(repo().join("dev/install-test.sh"))
        .output()
        .expect("bash runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
