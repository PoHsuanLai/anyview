use super::*;

fn code(source: &str, name: &str) -> (Highlighter, CodeLines<Vec<u8>>) {
    let highlighter = Highlighter::new();
    let syntax = highlighter.syntax(&SyntaxName::new(name).unwrap());
    let lines = TextLines::open(Vec::<u8>::from(source)).unwrap();
    let code = CodeLines::new(&highlighter, lines, syntax);
    (highlighter, code)
}

fn highlight(
    highlighter: &Highlighter,
    code: &mut CodeLines<Vec<u8>>,
    from: u32,
    to: u32,
) -> Vec<TokenLine> {
    code.highlight(highlighter, LineIndex(from)..LineIndex(to))
        .unwrap()
}

fn class_of_text(line: &TokenLine, text: &str) -> Option<TokenClass> {
    line.spans
        .iter()
        .find(|span| span.text.trim() == text)
        .map(|span| span.class)
}

#[test]
fn rust_tokens_get_their_semantic_classes() {
    let (h, mut c) = code("let x = 42; // answer\n", "rust");
    let lines = highlight(&h, &mut c, 0, 1);
    let line = &lines[0];
    assert_eq!(class_of_text(line, "let"), Some(TokenClass::Keyword));
    assert_eq!(class_of_text(line, "42"), Some(TokenClass::Number));
    assert!(
        line.spans
            .iter()
            .any(|s| s.class == TokenClass::Comment && s.text.contains("answer")),
        "{line:?}"
    );
    assert_eq!(line.text(), "let x = 42; // answer");
}

#[test]
fn strings_functions_and_types_are_told_apart() {
    let (h, mut c) = code(
        "fn greet(name: &str) -> String {\n    \"hi\".to_owned()\n}\n",
        "rust",
    );
    let lines = highlight(&h, &mut c, 0, 3);
    assert_eq!(class_of_text(&lines[0], "fn"), Some(TokenClass::Keyword));
    assert_eq!(
        class_of_text(&lines[0], "greet"),
        Some(TokenClass::Function)
    );
    assert!(
        lines[1].spans.iter().any(|s| s.class == TokenClass::String),
        "{:?}",
        lines[1]
    );
    for line in &lines {
        let joined: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, line.text());
    }
}

#[test]
fn python_and_html_use_their_own_syntaxes() {
    let (h, mut c) = code("def f():\n    return None\n", "python");
    let lines = highlight(&h, &mut c, 0, 2);
    assert_eq!(class_of_text(&lines[0], "def"), Some(TokenClass::Keyword));
    assert_eq!(class_of_text(&lines[1], "None"), Some(TokenClass::Constant));
    let (h, mut c) = code("<p class=\"a\">hi</p>\n", "html");
    let lines = highlight(&h, &mut c, 0, 1);
    assert!(
        lines[0].spans.iter().any(|s| s.class == TokenClass::Tag),
        "{:?}",
        lines[0]
    );
}

/// A file whose block comment opens on line 126 and closes on line 131, across the save point at
/// line 128.
fn commented_file() -> String {
    (0..200)
        .map(|i| match i {
            126 => "/* the comment opens".to_owned(),
            127..=130 => format!("still inside {i}"),
            131 => "closes here */ let after = 1;".to_owned(),
            _ => format!("let v{i} = {i};"),
        })
        .map(|line| line + "\n")
        .collect()
}

#[test]
fn a_window_inside_a_block_comment_is_a_comment_whichever_window_asked_first() {
    // The window right after a save point, asked before anything else.
    let (h, mut c) = code(&commented_file(), "rust");
    let direct = highlight(&h, &mut c, 129, 131);
    assert!(
        direct
            .iter()
            .all(|l| l.spans.iter().all(|s| s.class == TokenClass::Comment)),
        "{direct:?}"
    );
    // The same window after highlighting the file from the top.
    let (h, mut c) = code(&commented_file(), "rust");
    let whole = highlight(&h, &mut c, 0, 200);
    assert_eq!(&whole[129..131], &direct[..]);
    let again = highlight(&h, &mut c, 129, 131);
    assert_eq!(again, direct, "a window after the save points exist");
    // After the comment closes the code is code again.
    assert_eq!(class_of_text(&whole[131], "let"), Some(TokenClass::Keyword));
}

#[test]
fn every_window_equals_the_same_lines_of_a_whole_pass() {
    let source = commented_file();
    let (h, mut c) = code(&source, "rust");
    let whole = highlight(&h, &mut c, 0, 200);
    assert_eq!(whole.len(), 200);
    for (from, to) in [(0, 5), (127, 129), (128, 129), (190, 200), (63, 260)] {
        let (h, mut fresh) = code(&source, "rust");
        let part = highlight(&h, &mut fresh, from, to);
        let to = to.min(200) as usize;
        assert_eq!(part, whole[from as usize..to], "{from}..{to}");
    }
}

#[test]
fn a_language_without_a_syntax_is_shown_plain() {
    let (h, mut c) = code("answer = 42\nother\n", "vue");
    assert_eq!(h.syntax(&SyntaxName::new("vue").unwrap()), None);
    let lines = highlight(&h, &mut c, 0, 2);
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].spans,
        [TokenSpan {
            class: TokenClass::Plain,
            text: "answer = 42".to_owned()
        }]
    );
}

#[test]
fn a_very_long_line_is_one_plain_span_and_does_not_disturb_the_next() {
    let long = format!("let s = \"{}\";", "x".repeat(5000));
    let source = format!("{long}\nlet y = 1;\n");
    let (h, mut c) = code(&source, "rust");
    let lines = highlight(&h, &mut c, 0, 2);
    assert_eq!(lines[0].spans.len(), 1);
    assert_eq!(lines[0].spans[0].class, TokenClass::Plain);
    assert_eq!(class_of_text(&lines[1], "let"), Some(TokenClass::Keyword));
}

#[test]
fn syntax_names_find_the_syntaxes_with_aliases_where_the_names_differ() {
    // name, syntax name, found
    const CASES: &[(&str, &str, bool)] = &[
        ("same name", "rust", true),
        ("different case in the set", "javascript", true),
        ("shell alias", "shell", true),
        ("typescript has its own", "typescript", true),
        ("tsx alias", "tsx", true),
        ("kotlin has its own", "kotlin", true),
        ("scss has its own", "scss", true),
        ("protobuf alias", "protobuf", true),
        ("hcl alias", "hcl", true),
        ("markdown", "markdown", true),
        ("makefile", "makefile", true),
        ("toml", "toml", true),
        ("swift", "swift", true),
        ("vue is not in the set", "vue", false),
        ("unknown", "klingon", false),
    ];
    let h = Highlighter::new();
    for (name, syntax, found) in CASES {
        let got = h.syntax(&SyntaxName::new(syntax).unwrap());
        assert_eq!(got.is_some(), *found, "{name}");
    }
}

#[test]
fn a_snippet_highlights_without_a_file() {
    let h = Highlighter::new();
    let id = h.syntax(&SyntaxName::new("rust").unwrap()).unwrap();
    let lines = h.snippet(Some(id), "let a = 1;\nlet b = 2;");
    assert_eq!(lines.len(), 2);
    assert_eq!(class_of_text(&lines[1], "let"), Some(TokenClass::Keyword));
    assert_eq!(lines[1].number, LineIndex(1));
}

#[test]
fn a_snippet_without_a_syntax_is_plain_lines() {
    let h = Highlighter::new();
    let lines = h.snippet(None, "a\nb");
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1].spans,
        [TokenSpan {
            class: TokenClass::Plain,
            text: "b".to_owned()
        }]
    );
}

#[test]
fn a_range_past_the_end_is_cut() {
    let (h, mut c) = code("a\nb\n", "rust");
    assert_eq!(highlight(&h, &mut c, 1, 50).len(), 1);
    assert_eq!(highlight(&h, &mut c, 5, 9).len(), 0);
    assert_eq!(c.text().line_count().0, 2);
}

#[test]
fn the_added_languages_highlight_with_the_right_classes() {
    use TokenClass::*;
    // case, syntax name, source, (text, class) pairs that must appear in the first lines
    type Wanted = &'static [(&'static str, TokenClass)];
    const CASES: &[(&str, &str, &str, Wanted)] = &[
        (
            "toml",
            "toml",
            "# note\n[package]\nname = \"a\"\nn = 3\nok = true\n",
            &[
                ("# note", Comment),
                ("\"a\"", String),
                ("3", Number),
                ("true", Constant),
            ],
        ),
        (
            "typescript",
            "typescript",
            "interface A { x: number }\nconst f = (a: string): void => { return; }\n",
            &[
                ("interface", Keyword),
                ("A", Type),
                ("number", Type),
                ("const", Keyword),
                ("f", Function),
                ("return", Keyword),
            ],
        ),
        (
            "tsx",
            "tsx",
            "const e = <div className=\"a\">{x}</div>;\n",
            &[
                ("const", Keyword),
                ("<div", Tag),
                ("className", Attribute),
                ("\"a\"", String),
                ("</div>", Tag),
            ],
        ),
        (
            "kotlin",
            "kotlin",
            "fun main(args: Array<String>) {\n    val x = 1 // one\n}\n",
            &[
                ("fun", Keyword),
                ("main", Function),
                ("Array", Type),
                ("val", Keyword),
                ("1", Number),
                ("// one", Comment),
            ],
        ),
        (
            "scss",
            "scss",
            "$c: #fff; /* base */\n// line\n.a { &:hover { margin: 0 } }\n",
            &[
                ("/* base */", Comment),
                ("// line", Comment),
                ("c", Variable),
                ("fff", Constant),
                ("0", Number),
            ],
        ),
        (
            "dockerfile",
            "dockerfile",
            "FROM rust:1\nRUN cargo build\n",
            &[("FROM", Keyword), ("RUN", Keyword)],
        ),
        (
            "cmake",
            "cmake",
            "cmake_minimum_required(VERSION 3.0)\nproject(x)\n",
            &[
                ("cmake_minimum_required", Function),
                ("project", Function),
                ("VERSION", Variable),
            ],
        ),
        (
            "protobuf",
            "protobuf",
            "syntax = \"proto3\";\nmessage A { int32 x = 1; }\n",
            &[
                ("syntax", Keyword),
                ("\"proto3\"", String),
                ("message", Keyword),
                ("A", Type),
                ("1", Number),
            ],
        ),
        (
            "zig",
            "zig",
            "const std = @import(\"std\");\npub fn main() void {}\n",
            &[
                ("const", Keyword),
                ("@import", Keyword),
                ("\"std\"", String),
                ("main", Function),
            ],
        ),
        (
            "terraform",
            "terraform",
            "resource \"a\" \"b\" {\n  x = 1\n}\n",
            &[("resource", Keyword), ("1", Number)],
        ),
        (
            "nix",
            "nix",
            "{ pkgs }: pkgs.mkShell { a = 1; }\n",
            &[("1", Number), ("a", Attribute)],
        ),
        (
            "swift",
            "swift",
            "func f(a: Int) -> String { return \"x\" }\n",
            &[
                ("func", Keyword),
                ("Int", Type),
                ("return", Keyword),
                ("\"x\"", String),
            ],
        ),
        (
            "dart",
            "dart",
            "void main() { var x = 1; }\n",
            &[
                ("void", Keyword),
                ("main", Function),
                ("var", Keyword),
                ("1", Number),
            ],
        ),
        (
            "powershell",
            "powershell",
            "function Get-X { Write-Host \"hi\" }\n",
            &[
                ("function", Keyword),
                ("Get-X", Function),
                ("Write-Host", Function),
                ("\"hi\"", String),
            ],
        ),
        (
            "dotenv",
            "dotenv",
            "# c\nKEY=value\n",
            &[("# c", Comment), ("KEY", Variable)],
        ),
        (
            "ini",
            "ini",
            "; c\n[sec]\nkey = \"v\"\nn = 3\n",
            &[
                ("; c", Comment),
                ("key", Variable),
                ("\"v\"", String),
                ("3", Number),
            ],
        ),
        (
            "makefile (default set)",
            "makefile",
            "all: a.o\n\tcc -o a a.o\n",
            &[("all", Function)],
        ),
    ];
    let h = Highlighter::new();
    for (case, syntax, source, wanted) in CASES {
        let id = h.syntax(&SyntaxName::new(syntax).unwrap());
        assert!(id.is_some(), "{case}: the syntax is missing");
        let lines = h.snippet(id, source);
        for (text, class) in *wanted {
            let got = lines.iter().find_map(|line| class_of_text(line, text));
            assert_eq!(got, Some(*class), "{case}: {text}");
        }
    }
}

#[test]
fn a_typescript_file_highlights_the_same_in_any_window() {
    let source: String = (0..400)
        .map(|n| format!("export const value{n}: number = {n}; /* a\n   comment */\n"))
        .collect();
    let (h, mut whole) = code(&source, "typescript");
    let all = highlight(&h, &mut whole, 0, 800);
    let (h, mut windowed) = code(&source, "typescript");
    let tail = highlight(&h, &mut windowed, 700, 720);
    assert_eq!(tail, all[700..720]);
    assert_eq!(
        class_of_text(&tail[1], "comment */"),
        Some(TokenClass::Comment)
    );
    assert_eq!(class_of_text(&all[0], "export"), Some(TokenClass::Keyword));
}

#[test]
fn fenced_labels_find_the_added_languages() {
    let h = Highlighter::new();
    for label in [
        "toml",
        "ts",
        "typescript",
        "tsx",
        "kt",
        "scss",
        "dockerfile",
        "cmake",
        "zig",
        "swift",
        "dart",
        "ps1",
        "proto",
        "nix",
        "tf",
        "ini",
    ] {
        assert!(h.syntax_by_token(label).is_some(), "{label}");
    }
}

#[test]
fn the_packed_syntax_set_loads_with_the_default_and_the_added_syntaxes() {
    let h = Highlighter::new();
    assert!(h.set.syntaxes().len() > 90, "the dump did not load");
}
