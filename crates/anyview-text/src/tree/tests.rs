use super::*;
use anyview_core::work::Stop;
use anyview_core::{FileName, TreePath};

fn json(text: &str) -> Tree {
    Tree::parse(text, TreeFormat::Json).unwrap()
}

/// `(label, preview)` of the rows under `path`.
fn under(tree: &Tree, path: &TreePath) -> Vec<(String, String)> {
    tree.children(path, 0..u32::MAX)
        .unwrap()
        .into_iter()
        .map(|row| {
            let label = match row.label {
                RowLabel::Root => "root".to_owned(),
                RowLabel::Key(key) => key,
                RowLabel::Index(index) => format!("[{index}]"),
            };
            (label, row.preview)
        })
        .collect()
}

#[test]
fn the_root_summarises_the_document_and_children_open_one_level() {
    let tree = json(
        r#"{"name":"anyview","version":1.5,"deps":{"a":1,"b":2},"tags":["x","y","z"],"ok":true,"none":null}"#,
    );
    let root = tree.row(&TreePath::root()).unwrap();
    assert_eq!(root.label, RowLabel::Root);
    assert_eq!(root.preview, "{6 keys}");
    assert_eq!(root.children, ChildCount(6));
    let expected: Vec<(String, String)> = [
        ("name", "\"anyview\""),
        ("version", "1.5"),
        ("deps", "{2 keys}"),
        ("tags", "[3 items]"),
        ("ok", "true"),
        ("none", "null"),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();
    assert_eq!(under(&tree, &TreePath::root()), expected);
    let deps = TreePath::root().child(2);
    assert_eq!(
        under(&tree, &deps),
        [
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "2".to_owned())
        ]
    );
    let tags = TreePath::root().child(3);
    assert_eq!(
        under(&tree, &tags)[2],
        ("[2]".to_owned(), "\"z\"".to_owned())
    );
}

#[test]
fn a_row_is_labelled_by_the_step_that_leads_to_it() {
    let tree = json(r#"{"list":[10,20]}"#);
    let list = TreePath::root().child(0);
    assert_eq!(
        tree.row(&list).unwrap().label,
        RowLabel::Key("list".to_owned())
    );
    let second = list.child(1);
    let row = tree.row(&second).unwrap();
    assert_eq!(row.label, RowLabel::Index(1));
    assert_eq!(row.preview, "20");
}

#[test]
fn keys_stay_in_file_order_and_duplicates_are_both_kept() {
    let tree = json(r#"{"z":1,"a":2,"z":3}"#);
    assert_eq!(tree.top_level_keys(10), ["z", "a", "z"]);
    assert_eq!(tree.top_level_keys(2), ["z", "a"]);
    assert_eq!(
        under(&tree, &TreePath::root())[2],
        ("z".to_owned(), "3".to_owned())
    );
}

#[test]
fn children_are_windowed_and_cut_to_what_exists() {
    let text = format!(
        "[{}]",
        (0..1000)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let tree = json(&text);
    let window = tree.children(&TreePath::root(), 500..503).unwrap();
    let got: Vec<(RowLabel, String)> = window.into_iter().map(|r| (r.label, r.preview)).collect();
    assert_eq!(
        got,
        [
            (RowLabel::Index(500), "500".to_owned()),
            (RowLabel::Index(501), "501".to_owned()),
            (RowLabel::Index(502), "502".to_owned()),
        ]
    );
    assert_eq!(
        tree.children(&TreePath::root(), 998..5000).unwrap().len(),
        2
    );
    assert!(
        tree.children(&TreePath::root(), 2000..3000)
            .unwrap()
            .is_empty()
    );
    assert!(tree.children(&TreePath::root(), 7..7).unwrap().is_empty());
}

#[test]
fn a_scalar_has_no_children_and_a_wrong_path_is_an_error() {
    let tree = json(r#"{"a":1}"#);
    assert!(
        tree.children(&TreePath::root().child(0), 0..9)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        tree.row(&TreePath::root().child(5)),
        Err(TextError::NoSuchNode)
    );
    assert_eq!(
        tree.children(&TreePath::root().child(0).child(0), 0..9),
        Err(TextError::NoSuchNode)
    );
}

#[test]
fn scalar_roots_and_empty_containers_parse() {
    // name, text, preview, children
    const CASES: &[(&str, &str, &str, u32)] = &[
        ("number", "42", "42", 0),
        ("string", "\"hi\"", "\"hi\"", 0),
        ("null", "null", "null", 0),
        ("empty object", "{}", "{}", 0),
        ("empty array", "[]", "[]", 0),
        (
            "big integer",
            "18446744073709551615",
            "18446744073709551615",
            0,
        ),
        ("a whole float keeps its point", "1.0", "1.0", 0),
        ("a number keeps its trailing zeros", "1.50", "1.50", 0),
        ("an exponent is kept as written", "-1E+3", "-1E+3", 0),
    ];
    for (name, text, preview, children) in CASES {
        let row = json(text).row(&TreePath::root()).unwrap();
        assert_eq!(row.preview, *preview, "{name}");
        assert_eq!(row.children, ChildCount(*children), "{name}");
    }
}

#[test]
fn json_lines_become_the_items_of_an_array_skipping_blank_lines() {
    let tree = Tree::parse("{\"a\":1}\n\n[1,2]\n\"x\"\n", TreeFormat::JsonLines).unwrap();
    assert_eq!(tree.format(), TreeFormat::JsonLines);
    let root = tree.row(&TreePath::root()).unwrap();
    assert_eq!(root.preview, "[3 items]");
    assert_eq!(
        under(&tree, &TreePath::root()),
        [
            ("[0]".to_owned(), "{1 key}".to_owned()),
            ("[1]".to_owned(), "[2 items]".to_owned()),
            ("[2]".to_owned(), "\"x\"".to_owned()),
        ]
    );
}

#[test]
fn malformed_json_names_its_line_and_column() {
    // name, text, format, line (zero-based), column
    const CASES: &[(&str, &str, TreeFormat, u32, u32)] = &[
        ("one line", "{\"a\":}", TreeFormat::Json, 0, 6),
        ("second line", "{\n\"a\": ]\n}", TreeFormat::Json, 1, 6),
        ("trailing text", "1 2", TreeFormat::Json, 0, 3),
    ];
    for (name, text, format, line, column) in CASES {
        match Tree::parse(text, *format) {
            Err(TextError::Json {
                line: got_line,
                column: got_column,
                ..
            }) => {
                assert_eq!(
                    (got_line, got_column),
                    (LineIndex(*line), *column),
                    "{name}"
                );
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[test]
fn a_file_longer_than_the_limit_shows_its_start_and_a_deep_node_opens_when_asked() {
    let items: Vec<String> = (0..2000)
        .map(|n| format!("{{\"id\":{n},\"inner\":{{\"deep\":[{n},1.0]}}}}"))
        .collect();
    let text = format!("[{}]", items.join(",\n"));
    let input = Input::from((
        FileName::new("big.json").unwrap(),
        text.clone().into_bytes(),
    ));
    // name, limit, coverage, items counted
    let cases = [
        (
            "the whole file",
            text.len() as u64 + 1,
            Coverage::Whole,
            2000,
        ),
        ("a tenth of it", text.len() as u64 / 10, Coverage::Prefix, 0),
    ];
    for (name, limit, coverage, count) in cases {
        let (tree, got) = Tree::read(&input, TreeFormat::Json, ByteLen(limit)).unwrap();
        assert_eq!(got, coverage, "{name}: coverage");
        let root = tree.row(&TreePath::root()).unwrap();
        if count > 0 {
            assert_eq!(root.children, ChildCount(count), "{name}: items");
        } else {
            assert!(root.children.0 > 100, "{name}: the start is counted");
            assert!(root.children.0 < 2000, "{name}: the rest is not");
        }
        let at = root.children.0 - 1;
        let item = TreePath::root().child(at);
        let inner = item.child(1);
        let deep = inner.child(0);
        assert_eq!(
            under(&tree, &item)[0],
            ("id".to_owned(), at.to_string()),
            "{name}: an item opens"
        );
        assert_eq!(
            under(&tree, &deep),
            [
                ("[0]".to_owned(), at.to_string()),
                ("[1]".to_owned(), "1.0".to_owned())
            ],
            "{name}: a deep node opens"
        );
    }
}

#[test]
fn a_single_value_cut_by_the_limit_is_over_budget() {
    let input = Input::from((FileName::new("s.json").unwrap(), b"\"abcdefghij\"".to_vec()));
    assert_eq!(
        Tree::read(&input, TreeFormat::Json, ByteLen(5)).map(|(_, coverage)| coverage),
        Err(TextError::JsonOverBudget)
    );
}

#[test]
fn json_lines_past_the_limit_count_only_whole_lines() {
    let text = "1\n22\n333\n4444\n";
    // name, limit, coverage, lines counted
    const CASES: &[(&str, u64, Coverage, u32)] = &[
        ("all of it", 99, Coverage::Whole, 4),
        ("a cut inside a line", 8, Coverage::Prefix, 2),
        ("a cut after a line break", 9, Coverage::Prefix, 3),
    ];
    for (name, limit, coverage, count) in CASES {
        let input = Input::from((FileName::new("a.jsonl").unwrap(), text.as_bytes().to_vec()));
        let (tree, got) = Tree::read(&input, TreeFormat::JsonLines, ByteLen(*limit)).unwrap();
        assert_eq!(got, *coverage, "{name}: coverage");
        let root = tree.row(&TreePath::root()).unwrap();
        assert_eq!(root.children, ChildCount(*count), "{name}: lines");
    }
}

#[test]
fn a_bad_json_line_is_a_row_of_its_own_and_the_lines_around_it_are_read() {
    // name, text, rows
    const CASES: &[(&str, &str, &[&str])] = &[
        (
            "a bad line between two good ones",
            "{\"ok\":1}\nnot json\n{\"ok\":2}\n",
            &["{1 key}", "unreadable: not json", "{1 key}"],
        ),
        (
            "a line cut short, then a good one",
            "{\"a\":\n[1]\n",
            &["unreadable: {\"a\":", "[1 item]"],
        ),
        (
            "a bad last line with no line end and a CRLF before it",
            "1\r\nnope",
            &["1", "unreadable: nope"],
        ),
        ("only a bad line", "{bad}", &["unreadable: {bad}"]),
    ];
    for (name, text, want) in CASES {
        let tree = Tree::parse(text, TreeFormat::JsonLines).unwrap();
        let rows: Vec<String> = under(&tree, &TreePath::root())
            .into_iter()
            .map(|(_, preview)| preview)
            .collect();
        assert_eq!(rows, *want, "{name}");
    }
    let tree = Tree::parse("1\nnope\n", TreeFormat::JsonLines).unwrap();
    let rows = tree.children(&TreePath::root(), 0..2).unwrap();
    assert_eq!(rows[1].kind, NodeKind::Unreadable);
}

#[test]
fn nesting_opens_to_ten_thousand_levels_and_is_refused_past_the_limit() {
    // name, depth, opens
    const CASES: &[(&str, usize, bool)] = &[
        ("past the old limit", 300, true),
        ("ten thousand", 10_000, true),
        ("past the limit", grammar::MAX_DEPTH + 1, false),
    ];
    for (name, depth, opens) in CASES {
        let objects = format!("{}1{}", "{\"k\":".repeat(*depth), "}".repeat(*depth));
        let arrays = format!("{}{}", "[".repeat(*depth), "]".repeat(*depth));
        for (kind, text) in [("objects", objects), ("arrays", arrays)] {
            let parsed = Tree::parse(&text, TreeFormat::Json);
            assert_eq!(parsed.is_ok(), *opens, "{name}: {kind}");
            if let Err(error) = parsed {
                assert!(matches!(error, TextError::Json { .. }), "{name}: {kind}");
            }
        }
    }
}

#[test]
fn a_path_thousands_of_steps_down_is_found() {
    let depth = 3_000;
    let text = format!("{}1{}", "{\"k\":".repeat(depth), "}".repeat(depth));
    let tree = json(&text);
    let path = (0..depth).fold(TreePath::root(), |at, _| at.child(0));
    let row = tree.row(&path).unwrap();
    assert_eq!(row.preview, "1");
    assert_eq!(row.label, RowLabel::Key("k".to_owned()));
}

#[test]
fn bytes_are_decoded_before_they_are_parsed() {
    let tree = Tree::parse_bytes(b"\xEF\xBB\xBF{\"k\":\"caf\xC3\xA9\"}", TreeFormat::Json).unwrap();
    assert_eq!(
        under(&tree, &TreePath::root()),
        [("k".to_owned(), "\"café\"".to_owned())]
    );
}

#[test]
fn a_scan_told_to_stop_is_interrupted_not_a_cut_document() {
    let stop = Stop::new();
    stop.request();
    let input = Input::from((FileName::new("a.json").unwrap(), b"[1,2,3]".to_vec()));
    assert_eq!(
        Tree::read_until(&input, TreeFormat::Json, ByteLen(1 << 20), &stop).map(|(_, c)| c),
        Err(TextError::Interrupted)
    );
}
