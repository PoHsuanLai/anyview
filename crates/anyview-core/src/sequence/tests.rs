use super::*;
use crate::error::CoreError;
use crate::source::FilePath;

fn file(index: usize) -> FilePath {
    FilePath::new(format!("/photos/{index}.jpg")).unwrap()
}

/// A folder sequence of `len` files named `0.jpg`, `1.jpg`, … pointing at `at`.
fn sequence(len: usize, at: usize) -> Sequence {
    let entries = NonEmpty::from_vec((0..len).map(file).collect()).unwrap();
    let seq = Sequence::new(entries, SequenceOrigin::Selection);
    let position = seq.position_at(at);
    moved(seq, SequenceMove::To(position))
}

#[test]
fn moved_walks_and_stops_at_the_ends() {
    use SequenceMove::{First, Last, Next, Previous};
    // name, length, start, move, index after
    const CASES: &[(&str, usize, usize, SequenceMove, usize)] = &[
        ("next in the middle", 5, 2, Next, 3),
        ("next to the last", 5, 3, Next, 4),
        ("next stops at the end", 5, 4, Next, 4),
        ("previous in the middle", 5, 2, Previous, 1),
        ("previous to the first", 5, 1, Previous, 0),
        ("previous stops at the start", 5, 0, Previous, 0),
        ("first from the middle", 5, 3, First, 0),
        ("first at the first", 5, 0, First, 0),
        ("last from the middle", 5, 1, Last, 4),
        ("last at the last", 5, 4, Last, 4),
        ("next in a single file", 1, 0, Next, 0),
        ("previous in a single file", 1, 0, Previous, 0),
        ("last in a single file", 1, 0, Last, 0),
    ];
    for (name, len, start, mv, want) in CASES {
        let after = moved(sequence(*len, *start), *mv);
        assert_eq!(after.at().index(), *want, "{name}");
        assert_eq!(after.current(), &file(*want), "{name} current");
        assert_eq!(
            after.entries().count().get(),
            *len,
            "{name} keeps every entry"
        );
    }
}

#[test]
fn moved_to_a_position_goes_there_and_clamps_a_foreign_one() {
    let long = sequence(10, 0);
    // name, length, position's index in the longer sequence, index after
    const CASES: &[(&str, usize, usize, usize)] = &[
        ("an entry", 5, 3, 3),
        ("the first", 5, 0, 0),
        ("the last", 5, 4, 4),
        ("a position past the end", 5, 8, 4),
        ("a position past the end of a single file", 1, 6, 0),
    ];
    for (name, len, index, want) in CASES {
        let position = long.position_at(*index);
        let after = moved(sequence(*len, 0), SequenceMove::To(position));
        assert_eq!(after.at().index(), *want, "{name}");
    }
}

#[test]
fn a_position_is_clamped_when_the_sequence_makes_it() {
    let seq = sequence(3, 0);
    assert_eq!(seq.position_at(2).index(), 2);
    assert_eq!(seq.position_at(3).index(), 2);
    assert_eq!(seq.position_at(usize::MAX).index(), 2);
}

#[test]
fn neighbours_are_the_entries_either_side() {
    // name, length, index, previous index, next index
    type Row = (&'static str, usize, usize, Option<usize>, Option<usize>);
    const CASES: &[Row] = &[
        ("middle", 5, 2, Some(1), Some(3)),
        ("first", 5, 0, None, Some(1)),
        ("last", 5, 4, Some(3), None),
        ("second of two", 2, 1, Some(0), None),
        ("single file", 1, 0, None, None),
    ];
    for (name, len, at, previous, next) in CASES {
        let got = neighbours(&sequence(*len, *at));
        assert_eq!(got.previous, previous.map(file), "{name} previous");
        assert_eq!(got.next, next.map(file), "{name} next");
    }
}

#[test]
fn a_sequence_starts_at_the_file_it_was_opened_on() {
    let entries = || NonEmpty::from_vec((0..4).map(file).collect()).unwrap();
    let opened = Sequence::starting_at(entries(), &file(2), SequenceOrigin::Selection).unwrap();
    assert_eq!(opened.at().index(), 2);
    assert_eq!(opened.current(), &file(2));
    let missing = Sequence::starting_at(entries(), &file(9), SequenceOrigin::Selection);
    assert_eq!(
        missing,
        Err(CoreError::NotInSequence {
            path: file(9).as_path().to_path_buf()
        })
    );
}

#[test]
fn a_new_sequence_points_at_the_first_and_remembers_its_origin() {
    let folder = FilePath::new("/photos").unwrap();
    let entries = NonEmpty::from_vec((0..3).map(file).collect()).unwrap();
    let seq = Sequence::new(entries, SequenceOrigin::Folder(folder.clone()));
    assert_eq!(seq.at().index(), 0);
    assert_eq!(seq.origin(), &SequenceOrigin::Folder(folder));
    assert_ne!(seq.origin(), &SequenceOrigin::Results(ResultsId(1)));
}

#[test]
fn a_file_taken_out_leaves_the_walk_on_the_file_the_person_was_heading_for() {
    use Heading::{Back, Onward};
    // name, length, the file on, heading, index after (in the shorter list), or none left
    const CASES: &[(&str, usize, usize, Heading, Option<usize>)] = &[
        ("onward in the middle takes the next", 5, 2, Onward, Some(2)),
        ("onward at the start takes the next", 5, 0, Onward, Some(0)),
        (
            "onward at the end falls back to the previous",
            5,
            4,
            Onward,
            Some(3),
        ),
        ("back in the middle takes the previous", 5, 2, Back, Some(1)),
        ("back at the start takes the next", 5, 0, Back, Some(0)),
        ("back at the end takes the previous", 5, 4, Back, Some(3)),
        ("two files leave one", 2, 0, Onward, Some(0)),
        ("one file leaves none", 1, 0, Onward, None),
    ];
    for (name, len, at, heading, want) in CASES {
        let after = without_current(sequence(*len, *at), *heading);
        assert_eq!(after.as_ref().map(|seq| seq.at().index()), *want, "{name}");
        if let Some(after) = after {
            assert_eq!(after.entries().count().get(), len - 1, "{name} drops one");
            assert!(
                after.entries().iter().all(|entry| *entry != file(*at)),
                "{name}: the file is gone from the list"
            );
        }
    }
}
