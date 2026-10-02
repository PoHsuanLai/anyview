use super::*;
use anyview_core::{
    FilePath, NonEmpty, PageIndex, Permille, ResultsId, Resume, Sequence, SequenceOrigin, Zoom,
};
use anyview_platform::Handoff;

fn touch(dir: &std::path::Path, name: &str) -> FilePath {
    std::fs::write(dir.join(name), "x").unwrap();
    FilePath::new(dir.join(name)).unwrap()
}

#[test]
fn the_sequence_is_the_visible_files_of_the_folder_by_name_pointing_at_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let b = touch(dir.path(), "b.png");
    let a = touch(dir.path(), "a.png");
    let c = touch(dir.path(), "c.txt");
    touch(dir.path(), ".hidden");
    std::fs::create_dir(dir.path().join("folder")).unwrap();

    let sequence = sequence_around(&b).unwrap();
    let entries: Vec<&FilePath> = sequence.entries().iter().collect();
    assert_eq!(entries, vec![&a, &b, &c], "no hidden file and no folder");
    assert_eq!(sequence.current(), &b);
}

#[test]
fn a_file_that_is_not_there_or_a_folder_that_cannot_be_read_has_no_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let missing = FilePath::new(dir.path().join("gone.png")).unwrap();
    assert_eq!(sequence_around(&missing), None, "not among the files");
    let nowhere = FilePath::new(dir.path().join("no-such-folder/a.png")).unwrap();
    assert_eq!(sequence_around(&nowhere), None, "no folder");
    assert_eq!(
        Opening::around(missing.clone()),
        Opening {
            file: missing,
            sequence: None,
            resume: Resume::Nothing,
        },
        "the window still opens, to say the file is not there"
    );
}

#[test]
fn a_handed_file_opens_with_the_results_and_the_place_it_brought() {
    let dir = tempfile::tempdir().unwrap();
    let a = touch(dir.path(), "a.pdf");
    let b = touch(dir.path(), "b.pdf");
    touch(dir.path(), "c.pdf");
    let results = Sequence::starting_at(
        NonEmpty::from_vec(vec![b.clone(), a.clone()]).unwrap(),
        &a,
        SequenceOrigin::Results(ResultsId(3)),
    )
    .unwrap();
    let place = Resume::Pdf {
        page: PageIndex(2),
        offset: Permille(0),
        zoom: Zoom::Fit,
    };
    let opening = Opening::handed(Handoff {
        file: a.clone(),
        resume: place.clone(),
        sequence: Some(results.clone()),
    });
    assert_eq!(opening.file, a);
    assert_eq!(opening.resume, place);
    assert_eq!(
        opening.sequence,
        Some(results),
        "the results are the sequence, not the folder (which holds c.pdf too)"
    );
}
