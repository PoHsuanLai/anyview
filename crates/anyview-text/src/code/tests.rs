use super::*;
use crate::bytes::HeldBytes;

fn code(source: &str, name: &str) -> (Highlighter, CodeLines<HeldBytes>) {
    let highlighter = Highlighter::new();
    let syntax = highlighter.syntax(&SyntaxName::new(name).unwrap());
    let lines = TextLines::open(HeldBytes::new(source)).unwrap();
    let code = CodeLines::new(&highlighter, lines, syntax);
    (highlighter, code)
}

fn highlight(
    highlighter: &Highlighter,
    code: &mut CodeLines<HeldBytes>,
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
    let (h, mut c) = code("answer = 42\nother\n", "swift");
    assert_eq!(h.syntax(&SyntaxName::new("swift").unwrap()), None);
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
fn syntax_names_find_the_default_syntaxes_with_aliases_where_the_names_differ() {
    // name, syntax name, found
    const CASES: &[(&str, &str, bool)] = &[
        ("same name", "rust", true),
        ("different case in the set", "javascript", true),
        ("shell alias", "shell", true),
        ("typescript uses javascript", "typescript", true),
        ("markdown", "markdown", true),
        ("makefile", "makefile", true),
        ("toml has none", "toml", false),
        ("swift has none", "swift", false),
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
