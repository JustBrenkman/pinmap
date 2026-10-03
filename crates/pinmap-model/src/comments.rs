use crate::Format;

/// Whether rewriting this text would lose comments. JSON has none. For RON,
/// `//` and `/* */` count only outside string and char literals.
pub fn has_comments(text: &str, format: Format) -> bool {
    match format {
        Format::Json => false,
        Format::Ron => ron_has_comments(text.as_bytes()),
    }
}

fn ron_has_comments(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        if let Some(hashes) = raw_string_hashes(bytes, i) {
            i = skip_raw_string(bytes, i + 2 + hashes, hashes);
            continue;
        }
        match bytes[i] {
            b'/' if matches!(bytes.get(i + 1), Some(b'/' | b'*')) => return true,
            b'"' => i = skip_quoted(bytes, i + 1, b'"'),
            b'\'' => i = skip_quoted(bytes, i + 1, b'\''),
            _ => i += 1,
        }
    }
    false
}

/// Returns the index just past the closing `quote`, honoring backslash escapes.
fn skip_quoted(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// If a raw string (`r"`, `r#"`, ...) starts at `i`, returns its number of `#`.
fn raw_string_hashes(bytes: &[u8], i: usize) -> Option<usize> {
    let starts_word = i == 0 || !is_ident(bytes[i - 1]);
    if bytes[i] != b'r' || !starts_word {
        return None;
    }
    let hashes = bytes[i + 1..].iter().take_while(|&&b| b == b'#').count();
    (bytes.get(i + 1 + hashes) == Some(&b'"')).then_some(hashes)
}

/// Returns the index just past a raw string body that starts at `i`.
fn skip_raw_string(bytes: &[u8], mut i: usize, hashes: usize) -> usize {
    while i < bytes.len() {
        let closing = bytes[i + 1..].iter().take_while(|&&b| b == b'#').count() >= hashes;
        if bytes[i] == b'"' && closing {
            return i + 1 + hashes;
        }
        i += 1;
    }
    i
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ron_comments() {
        let cases = [
            ("(a: 1)", false),
            ("#![enable(implicit_some)]\n(a: 1)", false),
            ("// header\n(a: 1)", true),
            ("(a: 1) /* trailing */", true),
            ("(a: 1, // why\n)", true),
            ("(a: \"http://example.com\")", false),
            ("(a: \"/* not a comment */\")", false),
            ("(a: \"quote \\\" then // still string\")", false),
            ("(a: r#\"raw \" // still raw\"#)", false),
            ("(a: r\"//\")", false),
            ("(a: '/', b: '\"', c: 1)", false),
            ("(a: '\\'', b: \"x\") // after", true),
            ("(bar: \"x\") // after an identifier ending in r", true),
        ];
        for (text, expected) in cases {
            assert_eq!(has_comments(text, Format::Ron), expected, "{text}");
        }
    }

    #[test]
    fn json_never_has_comments() {
        assert!(!has_comments("{\"a\": \"// x\"}", Format::Json));
    }
}
