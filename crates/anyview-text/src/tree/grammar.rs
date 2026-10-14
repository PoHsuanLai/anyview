//! The shape of JSON: walking a value byte by byte, checking it is well formed and saying where it
//! ends, without building anything.

use super::cursor::Cursor;
use std::ops::Range;

/// How many containers may enclose a value. The scan keeps the open containers on the heap, one
/// byte each, so this bounds memory and not the stack.
pub(super) const MAX_DEPTH: usize = 100_000;

/// What a value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Shape {
    Object,
    Array,
    String,
    Number,
    True,
    False,
    Null,
    /// A line of a JSON Lines file that is not a value: its bytes, as the line holds them.
    Unreadable,
}

/// How the children of a level are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Form {
    /// `{"key": value, …}`
    Object,
    /// `[value, …]`
    Array,
    /// Values one after another with nothing but white space between: a JSON Lines file.
    Stream,
}

/// Why a scan stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Why {
    /// The bytes ended in the middle of a value.
    Eof,
    /// The byte at the fault is not what the grammar allows there.
    Bad(&'static str),
}

/// Where and why a scan stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fault {
    /// The offset of the offending byte, or of the end.
    pub at: u64,
    pub why: Why,
}

/// One child of a level: its key (for an object), and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Child {
    /// The bytes of the key, quotes included.
    pub key: Option<Range<u64>>,
    /// The bytes of the value.
    pub value: Range<u64>,
    pub shape: Shape,
}

pub(super) fn bad(at: u64, reason: &'static str) -> Fault {
    Fault {
        at,
        why: Why::Bad(reason),
    }
}

pub(super) fn eof(c: &Cursor<'_>) -> Fault {
    Fault {
        at: c.pos(),
        why: Why::Eof,
    }
}

/// The fault for the byte under the cursor not being what `reason` expects: the end of the bytes
/// when there is none.
pub(super) fn unexpected(c: &mut Cursor<'_>, reason: &'static str) -> Fault {
    match c.peek() {
        None => eof(c),
        Some(_) => bad(c.pos(), reason),
    }
}

/// What kind of value starts at the cursor, by its first byte.
pub(super) fn shape_at(c: &mut Cursor<'_>) -> Result<Shape, Fault> {
    match c.peek() {
        Some(b'{') => Ok(Shape::Object),
        Some(b'[') => Ok(Shape::Array),
        Some(b'"') => Ok(Shape::String),
        Some(b't') => Ok(Shape::True),
        Some(b'f') => Ok(Shape::False),
        Some(b'n') => Ok(Shape::Null),
        Some(b'-' | b'0'..=b'9') => Ok(Shape::Number),
        _ => Err(unexpected(c, "expected a value")),
    }
}

/// Moves past the value at the cursor, checking it is well formed. `depth` is how many containers
/// enclose it.
pub(super) fn skip_value(c: &mut Cursor<'_>, depth: usize) -> Result<Shape, Fault> {
    let shape = shape_at(c)?;
    match shape {
        Shape::Object => skip_container(c, Form::Object, depth)?,
        Shape::Array => skip_container(c, Form::Array, depth)?,
        Shape::String | Shape::Number | Shape::True | Shape::False | Shape::Null => {
            skip_scalar(c, shape)?;
        }
        // `shape_at` never says so.
        Shape::Unreadable => return Err(bad(c.pos(), "expected a value")),
    }
    Ok(shape)
}

/// The bytes that close a container of `form`.
pub(super) fn closing(form: Form) -> Option<u8> {
    match form {
        Form::Object => Some(b'}'),
        Form::Array => Some(b']'),
        Form::Stream => None,
    }
}

/// The form of the container a value of `shape` is, or `None` for a scalar.
fn container_form(shape: Shape) -> Option<Form> {
    match shape {
        Shape::Object => Some(Form::Object),
        Shape::Array => Some(Form::Array),
        Shape::String
        | Shape::Number
        | Shape::True
        | Shape::False
        | Shape::Null
        | Shape::Unreadable => None,
    }
}

/// Reads the key and colon of the child at the cursor when `form` has them. The cursor is then at
/// the child's value.
fn read_key(c: &mut Cursor<'_>, form: Form) -> Result<Option<Range<u64>>, Fault> {
    match form {
        Form::Object => {
            let start = c.pos();
            if c.peek() != Some(b'"') {
                return Err(unexpected(c, "expected a string key"));
            }
            skip_string(c)?;
            let key = start..c.pos();
            c.skip_ws();
            if c.peek() != Some(b':') {
                return Err(unexpected(c, "expected `:`"));
            }
            c.bump();
            c.skip_ws();
            Ok(Some(key))
        }
        Form::Array | Form::Stream => Ok(None),
    }
}

/// Starts the child at the cursor of a container of `form`: its key, then its value. A scalar
/// value is skipped and `None` is returned; a container is left unopened, and its form returned.
fn begin_child(c: &mut Cursor<'_>, form: Form) -> Result<Option<Form>, Fault> {
    read_key(c, form)?;
    let shape = shape_at(c)?;
    match container_form(shape) {
        Some(inner) => Ok(Some(inner)),
        None => {
            skip_scalar(c, shape)?;
            Ok(None)
        }
    }
}

/// Moves past the container at the cursor, which is at its opening bracket. The containers open
/// inside it are a stack on the heap, so how deep it nests does not depend on the thread's stack.
/// `depth` is how many containers enclose it.
fn skip_container(c: &mut Cursor<'_>, form: Form, depth: usize) -> Result<(), Fault> {
    let mut open: Vec<Form> = Vec::new();
    let mut form = form;
    // Whether the cursor is at the opening bracket of `form`, as against after one of its
    // children.
    let mut entering = true;
    loop {
        let mut closed = false;
        if entering {
            if depth + open.len() >= MAX_DEPTH {
                return Err(bad(c.pos(), "recursion limit exceeded"));
            }
            c.bump();
            c.skip_ws();
            if c.peek() == closing(form) {
                c.bump();
                closed = true;
            } else if let Some(inner) = begin_child(c, form)? {
                open.push(form);
                form = inner;
                continue;
            }
            entering = false;
        }
        if !closed {
            // After a child: another follows, or the container ends.
            c.skip_ws();
            match c.peek() {
                Some(b',') => {
                    c.bump();
                    c.skip_ws();
                    if let Some(inner) = begin_child(c, form)? {
                        open.push(form);
                        form = inner;
                        entering = true;
                    }
                    continue;
                }
                Some(byte) if Some(byte) == closing(form) => c.bump(),
                _ => return Err(unexpected(c, "expected `,` or a closing bracket")),
            }
        }
        // `form` is closed: the container it was in continues, or the scan is done.
        match open.pop() {
            Some(outer) => form = outer,
            None => return Ok(()),
        }
    }
}

/// Reads the child at the cursor: for an object its key and colon, then its value. Leaves the
/// cursor after the value. `depth` is how many containers enclose the child's value.
pub(super) fn read_child(c: &mut Cursor<'_>, form: Form, depth: usize) -> Result<Child, Fault> {
    let key = read_key(c, form)?;
    let start = c.pos();
    let shape = skip_value(c, depth)?;
    Ok(Child {
        key,
        value: start..c.pos(),
        shape,
    })
}

/// Moves past a scalar of `shape`.
fn skip_scalar(c: &mut Cursor<'_>, shape: Shape) -> Result<(), Fault> {
    match shape {
        Shape::String => skip_string(c),
        Shape::Number => skip_number(c),
        Shape::True => literal(c, b"true"),
        Shape::False => literal(c, b"false"),
        Shape::Null => literal(c, b"null"),
        Shape::Object | Shape::Array | Shape::Unreadable => Err(bad(c.pos(), "expected a value")),
    }
}

fn literal(c: &mut Cursor<'_>, word: &[u8]) -> Result<(), Fault> {
    for expected in word {
        if c.peek() != Some(*expected) {
            return Err(unexpected(c, "expected a value"));
        }
        c.bump();
    }
    Ok(())
}

/// Moves past the string at the cursor, quotes included.
fn skip_string(c: &mut Cursor<'_>) -> Result<(), Fault> {
    c.bump();
    loop {
        let rest = c.rest();
        if rest.is_empty() {
            return Err(eof(c));
        }
        let Some(i) = rest
            .iter()
            .position(|b| matches!(*b, b'"' | b'\\' | 0..=0x1F))
        else {
            let all = rest.len();
            c.advance(all);
            continue;
        };
        let found = rest[i];
        c.advance(i);
        match found {
            b'"' => {
                c.bump();
                return Ok(());
            }
            b'\\' => {
                c.bump();
                escape(c)?;
            }
            _ => return Err(bad(c.pos(), "control character in a string")),
        }
    }
}

/// Moves past what follows a backslash.
fn escape(c: &mut Cursor<'_>) -> Result<(), Fault> {
    match c.peek() {
        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
            c.bump();
            Ok(())
        }
        Some(b'u') => {
            c.bump();
            for _ in 0..4 {
                match c.peek() {
                    Some(byte) if byte.is_ascii_hexdigit() => c.bump(),
                    _ => return Err(unexpected(c, "expected a hexadecimal digit")),
                }
            }
            Ok(())
        }
        _ => Err(unexpected(c, "invalid escape")),
    }
}

/// Moves past a run of digits; says how many.
fn digits(c: &mut Cursor<'_>) -> usize {
    let mut total = 0;
    loop {
        let rest = c.rest();
        let n = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        let more = n > 0 && n == rest.len();
        c.advance(n);
        total += n;
        if !more {
            return total;
        }
    }
}

/// Moves past a number: `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`.
fn skip_number(c: &mut Cursor<'_>) -> Result<(), Fault> {
    if c.peek() == Some(b'-') {
        c.bump();
    }
    match c.peek() {
        Some(b'0') => c.bump(),
        Some(b'1'..=b'9') => {
            digits(c);
        }
        _ => return Err(unexpected(c, "expected a digit")),
    }
    if c.peek() == Some(b'.') {
        c.bump();
        if digits(c) == 0 {
            return Err(unexpected(c, "expected a digit"));
        }
    }
    if matches!(c.peek(), Some(b'e' | b'E')) {
        c.bump();
        if matches!(c.peek(), Some(b'+' | b'-')) {
            c.bump();
        }
        if digits(c) == 0 {
            return Err(unexpected(c, "expected a digit"));
        }
    }
    Ok(())
}
