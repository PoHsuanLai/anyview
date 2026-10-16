//! Percent-escapes in a path or URL reference.

/// `%20` and its kind decoded, bytes that are not valid percent escapes left as they are.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_escapes_decode_and_malformed_ones_stay() {
        const CASES: &[(&str, &str)] = &[
            ("a%20b", "a b"),
            ("%C3%A9", "é"),
            ("100%", "100%"),
            ("%zz", "%zz"),
            ("%2", "%2"),
            ("plain", "plain"),
        ];
        for (text, want) in CASES {
            assert_eq!(percent_decode(text), *want, "{text}");
        }
    }
}
