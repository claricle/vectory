//! Lexical scanning over serialized XML: the source envelope.
//!
//! The Ruby graft/pipeline keeps the HOST document's original envelope bytes
//! (declaration, comments, PIs, DOCTYPE before the root; whitespace, comments
//! and PIs after it) and re-serializes only the root subtree between them.
//! This module reproduces those two envelope slices without a full
//! re-serialization pass, byte-for-byte.

/// Splits serialized content into `(prefix, root-start byte offset)` where
/// `prefix` is everything before the root element start tag (XML
/// declaration, comments, PIs, DOCTYPE, whitespace — in source order).
///
/// Returns `None` when the content has no root element start tag.
#[must_use]
pub fn root_offset(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut cursor = 0usize;

    // Optional XML declaration (only meaningful at the very start).
    if bytes.starts_with(b"<?xml") {
        cursor = find(bytes, b"?>", 5)? + 2;
    }

    loop {
        skip_whitespace(content, &mut cursor);
        let rest = &bytes[cursor.min(bytes.len())..];
        if rest.is_empty() {
            return None;
        }
        if rest.starts_with(b"<!--") {
            cursor = find(bytes, b"-->", cursor + 4)? + 3;
        } else if rest.starts_with(b"<?") {
            cursor = find(bytes, b"?>", cursor + 2)? + 2;
        } else if rest.starts_with(b"<!DOCTYPE") {
            cursor = doctype_end(bytes, cursor + 9)?;
        } else if rest.starts_with(b"<![") {
            // CDATA / conditional sections cannot appear before the root in
            // well-formed XML; treat as no root.
            return None;
        } else if rest.starts_with(b"<") {
            // An element start tag: the name must follow immediately.
            let next = bytes.get(cursor + 1)?;
            if next.is_ascii_alphabetic() || *next == b'_' || *next == b':' {
                return Some(cursor);
            }
            return None;
        } else {
            return None;
        }
    }
}

/// The trailing envelope: whitespace, comments and processing instructions
/// at the very end of the content (the Ruby
/// `/(?:[ \t\r\n]|<!--...-->|<?...?>)*\z` slice).
#[must_use]
pub fn trailing_envelope(content: &str) -> &str {
    let bytes = content.as_bytes();
    let mut end = bytes.len();
    loop {
        // Trim trailing whitespace.
        while end > 0 && matches!(bytes[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
            end -= 1;
        }
        if end >= 3 && &bytes[end - 3..end] == b"-->" {
            // Find the matching `<!--`.
            if let Some(start) = find_last(bytes, b"<!--", 0) {
                if start + 4 <= end - 3 {
                    end = start;
                    continue;
                }
            }
            return &content[end..];
        }
        if end >= 2 && &bytes[end - 2..end] == b"?>" {
            // Find the matching `<?` for this PI (scan backwards).
            let mut start = end - 2;
            let mut found = None;
            while start > 0 {
                if let Some(pos) = content[..start].rfind("<?") {
                    // A PI cannot contain `?>`; the closest `<?` before the
                    // closing delimiter is its target.
                    found = Some(pos);
                    break;
                }
                start -= 1;
            }
            if let Some(pos) = found {
                end = pos;
                continue;
            }
            return &content[end..];
        }
        return &content[end..];
    }
}

fn skip_whitespace(content: &str, cursor: &mut usize) {
    while let Some(c) = content[*cursor..].chars().next() {
        if c.is_whitespace() {
            *cursor += c.len_utf8();
        } else {
            break;
        }
    }
}

/// Scans a DOCTYPE body (after `<!DOCTYPE`) to its closing `>`, honoring
/// quoted strings and `[...]` internal subsets.
fn doctype_end(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    let mut subset_depth = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' | b'\'' => {
                let quote = bytes[cursor];
                cursor = find(bytes, &[quote], cursor + 1)? + 1;
            }
            b'[' => {
                subset_depth += 1;
                cursor += 1;
            }
            b']' => {
                subset_depth = subset_depth.saturating_sub(1);
                cursor += 1;
            }
            b'>' if subset_depth == 0 => return Some(cursor + 1),
            _ => cursor += 1,
        }
    }
    Some(cursor)
}

fn find(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

fn find_last(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= bytes.len() {
        return None;
    }
    (from..=bytes.len().saturating_sub(needle.len()))
        .rev()
        .find(|&position| &bytes[position..position + needle.len()] == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_offset_with_full_prolog() {
        let content = "<?xml version=\"1.0\"?><!-- c --><?pi x?><!DOCTYPE svg [<!E a>]><svg/>";
        assert_eq!(root_offset(content), Some(content.find("<svg/>").unwrap()));
    }

    #[test]
    fn finds_root_offset_without_prolog() {
        assert_eq!(root_offset("<a/>"), Some(0));
    }

    #[test]
    fn rejects_rootless_content() {
        assert_eq!(root_offset("<!--only-->"), None);
        assert_eq!(root_offset(""), None);
    }

    #[test]
    fn trailing_envelope_collects_tail() {
        let content = "<a/> x \n<!-- t --><?p y?> \n";
        assert_eq!(trailing_envelope(content), " \n<!-- t --><?p y?> \n");
    }

    #[test]
    fn trailing_envelope_empty_without_tail() {
        assert_eq!(trailing_envelope("<a/></b>"), "");
    }
}
