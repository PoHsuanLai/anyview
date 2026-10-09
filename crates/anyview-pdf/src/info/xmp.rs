//! The few fields of an XMP packet that stand in for an Info dictionary that lacks them:
//! `dc:title`, `dc:creator`, `dc:description` and `pdf:Keywords`.

use roxmltree::{Document, Node};

/// What the packet gave.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Xmp {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
}

const DC: &str = "http://purl.org/dc/elements/1.1/";
const PDF: &str = "http://ns.adobe.com/pdf/1.3/";

/// The fields of the packet `bytes`, empty when it is not well-formed XML.
pub(super) fn read(bytes: &[u8]) -> Xmp {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Xmp::default();
    };
    let Ok(document) = Document::parse(text) else {
        return Xmp::default();
    };
    // A property is an element, or, for a simple value, an attribute of its description.
    let property = |namespace: &str, name: &str| {
        document
            .descendants()
            .find(|node| node.has_tag_name((namespace, name)))
            .and_then(first_text)
            .or_else(|| {
                document
                    .descendants()
                    .find_map(|node| node.attribute((namespace, name)))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
    };
    // `dc:creator` is an ordered list: the first name is the author.
    Xmp {
        title: property(DC, "title"),
        author: property(DC, "creator"),
        subject: property(DC, "description"),
        keywords: property(PDF, "Keywords"),
    }
}

/// The first non-empty text under `node`: a plain value, or the first `rdf:li` of a list or
/// language alternative.
fn first_text(node: Node<'_, '_>) -> Option<String> {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|text| text.text())
        .map(str::trim)
        .find(|text| !text.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKET: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
      xmlns:pdf="http://ns.adobe.com/pdf/1.3/" pdf:Keywords="alpha, beta">
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">XMP Title</rdf:li></rdf:Alt></dc:title>
   <dc:creator><rdf:Seq><rdf:li>First Author</rdf:li><rdf:li>Second</rdf:li></rdf:Seq></dc:creator>
   <dc:description><rdf:Alt><rdf:li xml:lang="x-default">About it</rdf:li></rdf:Alt></dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

    #[test]
    fn the_packet_gives_title_first_author_and_description() {
        let got = read(PACKET.as_bytes());
        assert_eq!(got.title.as_deref(), Some("XMP Title"));
        assert_eq!(got.author.as_deref(), Some("First Author"));
        assert_eq!(got.subject.as_deref(), Some("About it"));
        assert_eq!(got.keywords.as_deref(), Some("alpha, beta"));
    }

    #[test]
    fn garbage_gives_nothing() {
        assert_eq!(read(b"not xml <<"), Xmp::default());
        assert_eq!(read(&[0xFF, 0xFE, 0x00]), Xmp::default());
    }
}
