//! The two questions asked of an XML part: the text of an element and the value of an attribute,
//! by local name, so a namespace prefix never matters.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

fn is(start: &BytesStart<'_>, local: &str) -> bool {
    start.local_name().as_ref() == local.as_bytes()
}

/// The text of the first element called `local`, trimmed; `None` when there is none or it is
/// empty.
pub(super) fn element_text(xml: &[u8], local: &str) -> Option<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut inside = false;
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(start) if !inside && is(&start, local) => inside = true,
            Event::Text(chunk) if inside => text.push_str(&chunk.decode().ok()?),
            Event::GeneralRef(entity) if inside => {
                if let Ok(Some(c)) = entity.resolve_char_ref() {
                    text.push(c);
                } else {
                    match entity.decode().ok()?.as_ref() {
                        "amp" => text.push('&'),
                        "lt" => text.push('<'),
                        "gt" => text.push('>'),
                        "quot" => text.push('"'),
                        "apos" => text.push('\''),
                        _ => {}
                    }
                }
            }
            Event::End(_) if inside => break,
            Event::Eof => break,
            Event::Start(_)
            | Event::End(_)
            | Event::Empty(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::GeneralRef(_) => {}
        }
        buf.clear();
    }
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// The value of attribute `attribute` on the first element called `element`.
pub(super) fn attribute(xml: &[u8], element: &str, attribute: &str) -> Option<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(start) | Event::Empty(start) if is(&start, element) => {
                return start
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attr| attr.key.local_name().as_ref() == attribute.as_bytes())
                    .map(|attr| String::from_utf8_lossy(&attr.value).into_owned());
            }
            Event::Eof => return None,
            Event::Start(_)
            | Event::End(_)
            | Event::Empty(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::GeneralRef(_) => {}
        }
        buf.clear();
    }
}

/// How many elements called `local` the part holds.
pub(super) fn count_elements(xml: &[u8], local: &str) -> u32 {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut found = 0u32;
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(start) | Event::Empty(start) if is(&start, local) => found += 1,
            Event::Eof => break,
            Event::Start(_)
            | Event::End(_)
            | Event::Empty(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::GeneralRef(_) => {}
        }
        buf.clear();
    }
    found
}
