//! The shape of JSON: walking a value byte by byte, checking it is well formed and saying where it
//! ends, without building anything.

use super::cursor::Cursor;
use std::ops::Range;

/// How many containers may enclose a value.
pub(super) const MAX_DEPTH: usize = 128;

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
        Shape::String => skip_string(c)?,
        Shape::Number => skip_number(c)?,
        Shape::True => literal(c, b"true")?,
        Shape::False => literal(c, b"false")?,
        Shape::Null => literal(c, b"null")?,
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

fn skip_container(c: &mut Cursor<'_>, form: Form, depth: usize) -> Result<(), Fault> {
    if depth >= MAX_DEPTH {
        return Err(bad(c.pos(), "recursion limit exceeded"));
    }
    let close = closing(form).unwrap_or(b']');
    c.bump();
    c.skip_ws();
    if c.peek() == Some(close) {
        c.bump();
        return Ok(());
    }
    loop {
        read_child(c, form, depth + 1)?;
        c.skip_ws();
        match c.peek() {
            Some(b',') => {
                c.bump();
                c.skip_ws();
            }
            Some(byte) if byte == close => {
                c.bump();
                return Ok(());
            }
            _ => return Err(unexpected(c, "expected `,` or a closing bracket")),
        }
    }
}

/// Reads the child at the cursor: for an object its key and colon, then its value. Leaves the
/// cursor after the value. `depth` is how many containers enclose the child's value.
pub(super) fn read_child(c: &mut Cursor<'_>, form: Form, depth: usize) -> Result<Child, Fault> {
    let key = match form {
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
            Some(key)
        }
        Form::Array | Form::Stream => None,
    };
    let start = c.pos();
    let shape = skip_value(c, depth)?;
    Ok(Child {
        key,
        value: start..c.pos(),
        shape,
    })
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
