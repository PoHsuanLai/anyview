use crate::host::{Declined, Doing, Outcome, notice_of, notice_of_declined};
use anyview_core::FilePath;

fn at(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

#[test]
fn what_the_person_is_told_follows_what_was_done_and_how_it_ended() {
    let photo = at("/p/photo.png");
    let copy = at("/p/photo 2.png");
    // name, what was done, how it ended, the words, whether it offers Show in Folder
    let cases: Vec<(&str, Doing, Outcome, Option<&str>, bool)> = vec![
        (
            "an export says where it went and offers the folder",
            Doing::Export,
            Outcome::Wrote(copy.clone()),
            Some("Exported as \u{201c}photo 2.png\u{201d}"),
            true,
        ),
        (
            "a copy says the name it took",
            Doing::SaveCopy,
            Outcome::Wrote(copy.clone()),
            Some("Saved a copy as \u{201c}photo 2.png\u{201d}"),
            true,
        ),
        (
            "a duplicate says the name it took",
            Doing::Duplicate,
            Outcome::Wrote(copy.clone()),
            Some("Duplicated as \u{201c}photo 2.png\u{201d}"),
            true,
        ),
        (
            "a failed export is told without the system's words",
            Doing::Export,
            Outcome::Failed("cannot write the export: os error 28".to_owned()),
            Some("Couldn\u{2019}t export \u{201c}photo.png\u{201d}"),
            false,
        ),
        (
            "the trash",
            Doing::Trash,
            Outcome::Done,
            Some("Moved \u{201c}photo.png\u{201d} to the Trash"),
            false,
        ),
        (
            "a trash that failed",
            Doing::Trash,
            Outcome::Failed("x".to_owned()),
            Some("Couldn\u{2019}t move \u{201c}photo.png\u{201d} to the Trash"),
            false,
        ),
        (
            "a rename names the new name",
            Doing::Rename,
            Outcome::Moved(at("/p/b.png")),
            Some("Renamed to \u{201c}b.png\u{201d}"),
            false,
        ),
        (
            "a rename onto a file that is there",
            Doing::Rename,
            Outcome::Taken,
            Some("A file with that name already exists. Choose another name."),
            false,
        ),
        (
            "no program to open with",
            Doing::OpenWith,
            Outcome::Nothing("none"),
            Some("No other app can open this kind of file"),
            false,
        ),
        (
            "no way to share",
            Doing::Share,
            Outcome::Nothing("none"),
            Some("Sharing is not available on this desktop"),
            false,
        ),
        (
            "no print dialog",
            Doing::Print,
            Outcome::Nothing("none"),
            Some("There is no print dialog on this desktop"),
            false,
        ),
        (
            "a print that ended is quiet",
            Doing::Print,
            Outcome::Done,
            None,
            false,
        ),
        (
            "a save that wrote nothing says the file is unchanged",
            Doing::Save,
            Outcome::NotWritten("x".to_owned()),
            Some("Couldn\u{2019}t save the change. \u{201c}photo.png\u{201d} is unchanged"),
            false,
        ),
    ];
    for (name, doing, outcome, words, reveals) in cases {
        let notice = notice_of(doing, Some(&photo), &outcome);
        assert_eq!(
            notice.as_ref().map(|notice| notice.text.as_str()),
            words,
            "{name}"
        );
        assert_eq!(
            notice.is_some_and(|notice| notice.reveal.is_some()),
            reveals,
            "{name}: Show in Folder"
        );
    }
}

#[test]
fn a_declined_request_is_told_only_when_the_person_should_hear() {
    // name, why, the words
    const CASES: &[(&str, Declined, Option<&str>)] = &[
        (
            "a bad name",
            Declined::NotAFileName,
            Some("That is not a valid file name"),
        ),
        (
            "nothing to undo",
            Declined::NothingToUndo,
            Some("Nothing to undo"),
        ),
        (
            "nothing to redo",
            Declined::NothingToRedo,
            Some("Nothing to redo"),
        ),
        ("waiting for a save", Declined::Queued, None),
        ("no file", Declined::NoFileShown, None),
    ];
    for (name, why, words) in CASES {
        assert_eq!(
            notice_of_declined(*why)
                .as_ref()
                .map(|notice| notice.text.as_str()),
            *words,
            "{name}"
        );
    }
}
