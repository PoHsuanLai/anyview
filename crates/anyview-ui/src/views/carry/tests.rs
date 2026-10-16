//! Tests of what the window carries out.

use super::out::copy_name;
use anyview_core::FilePath;

#[test]
fn a_copy_is_named_after_its_file() {
    // name, file, proposed name
    const CASES: &[(&str, &str, &str)] = &[
        ("with an extension", "/p/photo.png", "photo copy.png"),
        ("with none", "/p/notes", "notes copy"),
        ("with two dots", "/p/a.tar.gz", "a.tar copy.gz"),
    ];
    for (name, file, want) in CASES {
        let proposed = copy_name(&FilePath::new(file).unwrap());
        assert_eq!(proposed.as_str(), *want, "{name}");
    }
}
