//! The sequence a file belongs to by being in a folder: the files beside it, in name order. The
//! window lists a dropped file's folder through this; the binary lists the folder of a file named
//! on its command line through the same function, so the arrow keys walk the same list wherever
//! the file came from.

use super::error::OpenError;
use anyview_core::{FilePath, NonEmpty, Sequence, SequenceOrigin};
use std::cmp::Ordering;

/// The files of the folder `path` is in, as the sequence the arrow keys walk, pointing at `path`.
/// Hidden files and sub-folders are left out. When `path` is itself a folder, or is not among
/// what the listing found, the sequence is that one file alone. Blocking.
pub fn folder_sequence(path: &FilePath) -> Result<Sequence, OpenError> {
    let alone = || {
        Sequence::new(
            NonEmpty::new(path.clone(), vec![]),
            SequenceOrigin::Selection,
        )
    };
    let Some(parent) = path.parent() else {
        return Ok(alone());
    };
    let listing = std::fs::read_dir(parent.as_path()).map_err(|e| OpenError::Read(e.kind()))?;
    let mut entries: Vec<FilePath> = listing
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| FilePath::new(entry.path()).ok())
        .filter(|entry| !is_hidden(entry))
        .collect();
    entries.sort_by(name_order);
    let origin = SequenceOrigin::Folder(parent);
    Ok(NonEmpty::from_vec(entries)
        .and_then(|entries| Sequence::starting_at(entries, path, origin).ok())
        .unwrap_or_else(alone))
}

fn is_hidden(path: &FilePath) -> bool {
    path.file_name()
        .is_some_and(|name| name.as_str().starts_with('.'))
}

/// Names compared without regard to case, digits as numbers (`img2` before `img10`), and the
/// exact bytes breaking a tie so the order is total.
fn name_order(a: &FilePath, b: &FilePath) -> Ordering {
    let name = |path: &FilePath| {
        path.file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned())
    };
    let (a, b) = (name(a), name(b));
    natural(&a.to_lowercase(), &b.to_lowercase()).then_with(|| a.cmp(&b))
}

/// `a` against `b`, runs of digits compared as numbers and everything else by character.
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                let order = number_order(&x, &y);
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.cmp(&y);
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        run.push(c);
    }
    run
}

/// Two digit runs as numbers, however long: shorter after leading zeros is smaller.
fn number_order(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> FilePath {
        FilePath::new(format!("/p/{name}")).unwrap()
    }

    #[test]
    fn names_sort_the_way_a_person_counts() {
        // name, the names in the order given, the order they walk in
        const CASES: &[(&str, &[&str], &[&str])] = &[
            (
                "numbers are numbers",
                &["img10", "img2", "img1"],
                &["img1", "img2", "img10"],
            ),
            (
                "case is ignored",
                &["b.txt", "A.txt", "c.txt"],
                &["A.txt", "b.txt", "c.txt"],
            ),
            (
                "leading zeros do not count",
                &["a010", "a9"],
                &["a9", "a010"],
            ),
            ("a prefix comes first", &["ab", "a"], &["a", "ab"]),
            (
                "a tie is broken by the exact name",
                &["a", "A"],
                &["A", "a"],
            ),
        ];
        for (name, given, want) in CASES {
            let mut paths: Vec<FilePath> = given.iter().map(|n| path(n)).collect();
            paths.sort_by(name_order);
            let got: Vec<String> = paths
                .iter()
                .map(|p| p.file_name().unwrap().as_str().to_owned())
                .collect();
            assert_eq!(&got, want, "{name}");
        }
    }

    #[test]
    fn a_folder_lists_its_visible_files_in_order_and_points_at_the_one_given() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b10.png", "b2.png", ".hidden.png", "a.txt"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let given = FilePath::new(root.join("b2.png")).unwrap();
        let sequence = folder_sequence(&given).unwrap();
        let names: Vec<String> = sequence
            .entries()
            .iter()
            .map(|p| p.file_name().unwrap().as_str().to_owned())
            .collect();
        assert_eq!(names, ["a.txt", "b2.png", "b10.png"]);
        assert_eq!(sequence.current(), &given);
        assert_eq!(
            sequence.origin(),
            &SequenceOrigin::Folder(FilePath::new(&root).unwrap())
        );
    }

    #[test]
    fn a_file_the_listing_does_not_hold_is_a_sequence_of_one() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let hidden = root.join(".only");
        std::fs::write(&hidden, b"x").unwrap();
        let given = FilePath::new(&hidden).unwrap();
        let sequence = folder_sequence(&given).unwrap();
        assert_eq!(sequence.entries().count().get(), 1);
        assert_eq!(sequence.current(), &given);
    }
}
