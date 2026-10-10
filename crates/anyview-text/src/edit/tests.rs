use super::{EditRefusal, EditText, Motion, Selection, Session};
use crate::find::Needle;
use anyview_core::LineIndex;

fn session(text: &str) -> Session {
    Session::open(EditText::from_bytes(text.as_bytes().to_vec()).unwrap())
}

#[test]
fn a_buffer_reads_back_its_lines() {
    // name, text, line count, (line, its text, where it starts)
    type Case = (
        &'static str,
        &'static str,
        usize,
        &'static [(usize, &'static str, usize)],
    );
    const CASES: &[Case] = &[
        ("empty text has an empty line", "", 1, &[(0, "", 0)]),
        (
            "lines are cut at the break",
            "one\ntwo\nthree",
            3,
            &[(0, "one", 0), (1, "two", 4), (2, "three", 8)],
        ),
        (
            "a final break starts an empty line",
            "a\n",
            2,
            &[(0, "a", 0), (1, "", 2)],
        ),
        (
            "a CRLF break goes with its line",
            "a\r\nbc\r\n",
            3,
            &[(0, "a", 0), (1, "bc", 3), (2, "", 7)],
        ),
        ("a line past the last is the end", "ab", 1, &[(5, "", 2)]),
    ];
    for (name, text, count, lines) in CASES {
        let s = session(text);
        assert_eq!(s.buffer().line_count(), *count, "{name}: count");
        for (line, want, start) in *lines {
            assert_eq!(s.buffer().line_text(*line), *want, "{name}: line {line}");
            assert_eq!(s.buffer().line_start(*line), *start, "{name}: start {line}");
        }
    }
    let s = session("one\ntwo\r\nthree\n");
    assert_eq!(s.buffer().lines(1, 2), ["two", "three"]);
    assert_eq!(
        s.buffer().lines(2, 9),
        ["three", ""],
        "cut to the lines there are"
    );
    assert_eq!(s.buffer().lines(0, 0), Vec::<String>::new());
    assert_eq!(s.buffer().line_of(5), 1);
}

#[test]
fn edits_move_lines_without_rereading_the_text_and_never_cut_a_character() {
    let mut s = session("one\ntwo\nthree");
    s.set_caret(4);
    s.paste("new\n");
    assert_eq!(s.buffer().text(), "one\nnew\ntwo\nthree");
    assert_eq!(s.buffer().line_count(), 4);
    assert_eq!(s.buffer().line_text(2), "two");
    s.select(0, 8);
    s.cut();
    assert_eq!(s.buffer().text(), "two\nthree");
    assert_eq!((s.buffer().line_count(), s.buffer().line_of(4)), (2, 1));
    let mut s = session("日本語\nabc");
    s.set_caret(1); // inside 日: snaps back to its start
    assert_eq!(s.caret(), 0);
    s.select(1, 4);
    s.type_text("x");
    assert_eq!(
        s.buffer().text(),
        "x本語\nabc",
        "the range snapped to 日's bounds"
    );
    let big = "line\n".repeat(200_000);
    let mut s = session(&big);
    assert_eq!(s.buffer().line_count(), 200_001);
    s.set_caret(5 * 100_000);
    s.paste("mid\n");
    assert_eq!(s.buffer().line_count(), 200_002);
    assert_eq!(s.buffer().line_text(100_000), "mid");
    assert_eq!(s.buffer().len(), big.len() + 4);
}

/// One thing a person does in the table of undo steps.
#[derive(Clone, Copy)]
enum Do {
    Type(&'static str),
    Enter,
    Back,
    Del,
    Paste(&'static str),
    Cut,
    Caret(usize),
    Select(usize, usize),
    Move(Motion),
}

fn perform(s: &mut Session, acts: &[Do]) {
    for act in acts {
        match *act {
            Do::Type(text) => text.chars().for_each(|c| s.type_text(&c.to_string())),
            Do::Enter => s.enter(),
            Do::Back => s.backspace(),
            Do::Del => s.delete_forward(),
            Do::Paste(text) => s.paste(text),
            Do::Cut => {
                s.cut();
            }
            Do::Caret(at) => s.set_caret(at),
            Do::Select(a, f) => s.select(a, f),
            Do::Move(by) => s.move_by(by, false),
        }
    }
}

#[test]
fn undo_steps_group_the_way_editors_group_them() {
    use Do::{Back, Caret, Cut, Del, Enter, Move, Paste, Select, Type};
    // name, text at first, what is done, the text after the whole run, then the text after each undo
    type Case = (
        &'static str,
        &'static str,
        &'static [Do],
        &'static str,
        &'static [&'static str],
    );
    const CASES: &[Case] = &[
        (
            "typing a phrase: a word, its space, the next word",
            "",
            &[Type("hello world")],
            "hello world",
            &["hello ", "hello", ""],
        ),
        (
            "a caret move ends a run, even back where it was",
            "",
            &[
                Type("ab"),
                Move(Motion::Left),
                Move(Motion::Right),
                Type("c"),
            ],
            "abc",
            &["ab", ""],
        ),
        (
            "a line break is a step of its own",
            "",
            &[Type("ab"), Enter, Type("cd")],
            "ab\ncd",
            &["ab\n", "ab", ""],
        ),
        (
            "backspaces in a word are one step",
            "hello",
            &[Caret(5), Back, Back, Back],
            "he",
            &["hello"],
        ),
        (
            "backspaces are cut where the kind of character changes",
            "ab cd",
            &[Caret(5), Back, Back, Back, Back],
            "a",
            &["ab", "ab ", "ab cd"],
        ),
        (
            "deletes in a word are one step",
            "hello",
            &[Caret(0), Del, Del, Del],
            "lo",
            &["hello"],
        ),
        (
            "backspace over a line break is a step of its own",
            "ab\ncd",
            &[Caret(3), Back, Back],
            "acd",
            &["abcd", "ab\ncd"],
        ),
        (
            "a paste is a step, and ends the typing before it",
            "",
            &[Type("a"), Paste("bc"), Type("d")],
            "abcd",
            &["abc", "a", ""],
        ),
        (
            "typing over a selection replaces it as a step of its own",
            "abc",
            &[Select(0, 3), Type("x"), Type("y")],
            "xy",
            &["x", "abc"],
        ),
        (
            "a cut is a step",
            "abc def",
            &[Select(0, 4), Cut],
            "def",
            &["abc def"],
        ),
    ];
    for (name, start, acts, after, undone) in CASES {
        let mut s = session(start);
        perform(&mut s, acts);
        assert_eq!(s.buffer().text(), *after, "{name}: after");
        for (i, want) in undone.iter().enumerate() {
            s.undo();
            assert_eq!(s.buffer().text(), *want, "{name}: undo {}", i + 1);
        }
        let before = s.buffer().text();
        s.undo();
        assert_eq!(s.buffer().text(), before, "{name}: nothing more to undo");
    }
}

#[test]
fn redo_walks_forward_and_a_new_edit_ends_it() {
    let mut s = session("");
    perform(&mut s, &[Do::Type("one two")]);
    s.undo();
    s.undo();
    assert_eq!(s.buffer().text(), "one");
    s.redo();
    assert_eq!(s.buffer().text(), "one ");
    assert_eq!(s.caret(), 4, "the caret follows the redo");
    s.redo();
    assert_eq!(s.buffer().text(), "one two");
    s.redo();
    assert_eq!(s.buffer().text(), "one two", "nothing more to redo");
    s.undo();
    perform(&mut s, &[Do::Type("x")]);
    s.redo();
    assert_eq!(s.buffer().text(), "one x", "a new edit ends the redos");
}

#[test]
fn a_session_is_modified_exactly_when_its_text_differs_from_the_saved() {
    let mut s = session("abc");
    assert!(!s.is_modified(), "just opened");
    perform(&mut s, &[Do::Caret(3), Do::Type("d")]);
    assert!(s.is_modified(), "typed");
    s.undo();
    assert!(!s.is_modified(), "undone back to the opened text");
    perform(&mut s, &[Do::Type("d")]);
    let saved = s.revision();
    s.mark_saved(saved);
    assert!(!s.is_modified(), "saved");
    perform(&mut s, &[Do::Type("e")]);
    assert!(s.is_modified(), "typed after the save");
    assert_eq!(s.buffer().text(), "abcde");
    s.undo();
    assert_eq!(
        s.buffer().text(),
        "abcd",
        "typing after a save is a step of its own"
    );
    assert!(!s.is_modified(), "undone back to the saved text");
    s.undo();
    assert!(s.is_modified(), "undone past the saved text");
    s.redo();
    assert!(!s.is_modified(), "and redone to it");
    let stale = s.revision();
    perform(&mut s, &[Do::Type("f")]);
    s.mark_saved(stale);
    assert!(
        s.is_modified(),
        "a save of older text leaves the newer text unsaved"
    );
}

#[test]
fn the_caret_moves_by_grapheme_word_and_line() {
    let mut s = session("héllo wörld\nsecond line");
    s.move_by(Motion::WordRight, false);
    assert_eq!(s.caret(), "héllo".len());
    s.move_by(Motion::WordRight, false);
    assert_eq!(s.caret(), "héllo wörld".len());
    s.move_by(Motion::WordRight, false);
    assert_eq!(
        s.caret(),
        "héllo wörld\nsecond".len() - "second".len(),
        "to the next line"
    );
    s.move_by(Motion::WordLeft, false);
    assert_eq!(
        s.caret(),
        "héllo wörld".len(),
        "left from a line's start: the end of the line before"
    );
    s.move_by(Motion::WordLeft, false);
    assert_eq!(s.caret(), "héllo ".len());
    s.move_by(Motion::LineEnd, false);
    s.move_by(Motion::Right, false);
    assert_eq!(s.place(s.caret()), (1, 0), "right over the break");
    s.move_by(Motion::Left, false);
    assert_eq!(s.place(s.caret()), (0, "héllo wörld".len()));
    s.move_by(Motion::LineStart, false);
    assert_eq!(s.caret(), 0);
    s.move_by(Motion::DocEnd, false);
    assert_eq!(s.caret(), s.buffer().len());
}

#[test]
fn a_combined_mark_moves_and_deletes_as_one_grapheme() {
    let mut s = session("e\u{301}x");
    s.move_by(Motion::Right, false);
    assert_eq!(s.caret(), 3);
    s.backspace();
    assert_eq!(s.buffer().text(), "x");
}

#[test]
fn vertical_motion_keeps_its_column_through_a_short_line() {
    let mut s = session("abcdef\nab\nabcdef");
    s.set_caret(5);
    s.move_by(Motion::Down, false);
    assert_eq!(s.place(s.caret()), (1, 2));
    s.move_by(Motion::Down, false);
    assert_eq!(s.place(s.caret()), (2, 5), "the column comes back");
    s.move_by(Motion::Down, false);
    assert_eq!(s.caret(), s.buffer().len(), "past the last line: the end");
    s.move_by(Motion::DocStart, false);
    s.move_by(Motion::Up, false);
    assert_eq!(s.caret(), 0);
}

#[test]
fn shift_extends_and_a_plain_arrow_collapses_a_selection() {
    let mut s = session("hello world");
    s.move_by(Motion::WordRight, true);
    assert_eq!((s.selection().anchor, s.selection().focus), (0, 5));
    s.move_by(Motion::Right, true);
    assert_eq!(s.selection().range(), 0..6);
    s.move_by(Motion::Left, false);
    assert_eq!(
        s.selection(),
        Selection::caret(0),
        "Left goes to the near edge"
    );
    s.select(2, 7);
    s.move_by(Motion::Right, false);
    assert_eq!(s.caret(), 7);
}

#[test]
fn typing_replaces_the_selection_and_backspace_and_delete_join_lines() {
    let mut s = session("ab\ncd");
    s.select(1, 4);
    s.type_text("X");
    assert_eq!(s.buffer().text(), "aXd");
    let mut s = session("ab\ncd");
    s.set_caret(3);
    s.backspace();
    assert_eq!(s.buffer().text(), "abcd");
    assert_eq!(s.caret(), 2);
    let mut s = session("ab\r\ncd");
    s.set_caret(2);
    s.delete_forward();
    assert_eq!(s.buffer().text(), "abcd", "the whole CRLF goes");
    s.move_by(Motion::DocEnd, false);
    s.delete_forward();
    assert_eq!(s.buffer().text(), "abcd");
}

#[test]
fn a_line_break_follows_the_files_own_style() {
    // name, text, what is done, the text after
    const CASES: &[(&str, &str, &[Do], &str)] = &[
        (
            "Enter in a CRLF file",
            "a\r\nb",
            &[Do::Caret(1), Do::Enter],
            "a\r\n\r\nb",
        ),
        (
            "Enter in an LF file",
            "a\nb",
            &[Do::Caret(1), Do::Enter],
            "a\n\nb",
        ),
        (
            "a paste into a CRLF file",
            "a\r\nb",
            &[Do::Caret(1), Do::Paste("x\ny")],
            "ax\r\ny\r\nb",
        ),
        (
            "a paste of CRLF into an LF file",
            "a\nb",
            &[Do::Caret(1), Do::Paste("x\r\ny")],
            "ax\ny\nb",
        ),
    ];
    for (name, text, acts, want) in CASES {
        let mut s = session(text);
        perform(&mut s, acts);
        assert_eq!(s.buffer().text(), *want, "{name}");
    }
}

#[test]
fn cut_and_paste_round_trip_and_undo_as_one_step() {
    let mut s = session("hello world");
    s.select(0, 6);
    assert_eq!(s.selected_text().as_deref(), Some("hello "));
    assert_eq!(s.cut().as_deref(), Some("hello "));
    assert_eq!(s.buffer().text(), "world");
    s.move_by(Motion::DocEnd, false);
    s.paste("a\r\nb");
    assert_eq!(s.buffer().text(), "worlda\nb");
    s.undo();
    assert_eq!(s.buffer().text(), "world");
    s.set_caret(0);
    assert_eq!(s.cut(), None);
}

#[test]
fn a_composition_replaces_the_selection_then_commits_as_typed_text() {
    let mut s = session("abc");
    s.select(1, 2);
    s.compose("ㄓ", Some(3..3));
    assert_eq!(
        s.buffer().text(),
        "ac",
        "the selection went with the first preedit"
    );
    assert_eq!(s.preedit().map(|p| p.text.as_str()), Some("ㄓ"));
    s.compose("ㄓㄨ", None);
    s.compose("", None);
    assert!(s.preedit().is_none());
    s.end_composition("注");
    assert_eq!(s.buffer().text(), "a注c");
    assert_eq!(s.caret(), 1 + "注".len());
    s.compose("x", None);
    s.end_composition("");
    assert_eq!(
        s.buffer().text(),
        "a注c",
        "a cancelled composition inserts nothing"
    );
}

#[test]
fn words_and_lines_select_around_a_point() {
    let mut s = session("foo bar\nbaz");
    s.select_word(5);
    assert_eq!(s.selected_text().as_deref(), Some("bar"));
    s.select_line(1);
    assert_eq!(s.selected_text().as_deref(), Some("foo bar\n"));
    s.select_line(9);
    assert_eq!(s.selected_text().as_deref(), Some("baz"));
    s.select_all();
    assert_eq!(s.selected_text().as_deref(), Some("foo bar\nbaz"));
}

#[test]
fn a_find_reads_the_edited_text_not_the_file() {
    let mut s = session("cat\ndog\ncat");
    let needle = Needle::new("CAT").unwrap();
    let lines =
        |s: &Session| -> Vec<u32> { s.find(&needle).iter().map(|hit| hit.line.0).collect() };
    assert_eq!(lines(&s), [0, 2]);
    s.set_caret(3);
    s.type_text(" and cat");
    assert_eq!(lines(&s), [0, 0, 2], "a new hit on the line typed on");
    assert_eq!(s.find(&needle)[1].line, LineIndex(0));
    s.select_all();
    s.type_text("none here");
    assert!(s.find(&needle).is_empty());
}

#[test]
fn a_file_is_taken_for_editing_only_when_saving_it_back_changes_nothing_else() {
    // name, bytes, the bytes saved back unedited, or why not
    type Case = (
        &'static str,
        &'static [u8],
        Result<&'static [u8], EditRefusal>,
    );
    const CASES: &[Case] = &[
        ("ascii", b"hello\n", Ok(b"hello\n")),
        ("empty", b"", Ok(b"")),
        ("CRLF stays", b"a\r\nb\r\n", Ok(b"a\r\nb\r\n")),
        ("mixed endings stay", b"a\r\nb\nc", Ok(b"a\r\nb\nc")),
        (
            "a byte-order mark comes back",
            b"\xEF\xBB\xBFhi",
            Ok(b"\xEF\xBB\xBFhi"),
        ),
        (
            "UTF-16 is refused",
            b"\xFF\xFEh\0i\0",
            Err(EditRefusal::NotUtf8),
        ),
        ("Latin-1 is refused", b"caf\xE9", Err(EditRefusal::NotUtf8)),
        (
            "bad bytes after a mark are refused",
            b"\xEF\xBB\xBFa\xFFb",
            Err(EditRefusal::NotUtf8),
        ),
    ];
    for (name, bytes, want) in CASES {
        let got = EditText::from_bytes(bytes.to_vec()).map(|text| Session::open(text).file_bytes());
        let want = (*want).map(<[u8]>::to_vec);
        assert_eq!(got, want, "{name}");
    }
    let mut s = Session::open(EditText::from_bytes(b"\xEF\xBB\xBFhi".to_vec()).unwrap());
    s.set_caret(2);
    s.type_text("!");
    assert_eq!(
        s.file_bytes(),
        b"\xEF\xBB\xBFhi!",
        "the mark is kept over an edit"
    );
}

#[test]
fn a_file_is_read_whole_or_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "hé\n").unwrap();
    let text = EditText::read(&path).unwrap();
    assert_eq!(Session::open(text).buffer().text(), "hé\n");
    std::fs::write(&path, b"caf\xE9").unwrap();
    assert_eq!(EditText::read(&path), Err(EditRefusal::NotUtf8));
    assert_eq!(
        EditText::read(&dir.path().join("gone.txt")),
        Err(EditRefusal::Unreadable(std::io::ErrorKind::NotFound))
    );
    let big = vec![b'a'; usize::try_from(super::EDIT_BYTES).unwrap() + 1];
    std::fs::write(&path, big).unwrap();
    assert_eq!(EditText::read(&path), Err(EditRefusal::TooLarge));
}
