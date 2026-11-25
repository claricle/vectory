//! Port of lib/vectory/svg_input_policy.rb — inspect raw bytes before
//! parsing can discard a DTD or expand an entity. Also carries the
//! SVG-relevant half of lib/vectory/file_magic.rb (the `:svg` sniff);
//! the EPS/PS/EMF magic classifications belong to their own model crates
//! per the model-driven law.

use crate::error::Error;

/// Validates content before parsing: XML DOCTYPE internal subsets are
/// forbidden (they can declare entities), and the scan never loads any
/// external identifier. Literal declaration text inside comments, CDATA
/// and processing instructions is not markup and is skipped opaquely.
///
/// # Errors
///
/// Returns [`Error::ForbiddenExternalReference`] for internal DTD subsets.
pub fn validate(content: &str) -> Result<(), Error> {
    if forbidden(content.as_bytes()) {
        return Err(Error::ForbiddenExternalReference(
            "XML DOCTYPE internal subsets are forbidden".to_string(),
        ));
    }
    Ok(())
}

const DOCTYPE_PREFIX: &[u8] = b"<!DOCTYPE";

/// Literal text inside these tokens is not markup.
const OPAQUE: [(&[u8], &[u8]); 3] = [(b"<!--", b"-->"), (b"<![CDATA[", b"]]>"), (b"<?", b"?>")];

fn token_end(bytes: &[u8], opening: usize) -> usize {
    if bytes.len() >= opening + 10 && &bytes[opening..opening + 9] == DOCTYPE_PREFIX {
        let after = bytes[opening + 9];
        if after.is_ascii_whitespace() || after == b'[' || after == b'>' {
            return doctype_end(bytes, opening + 9);
        }
    }
    for (token, delimiter) in OPAQUE {
        if bytes.len() >= opening + token.len() && &bytes[opening..opening + token.len()] == token {
            return closing(bytes, delimiter, opening + token.len());
        }
    }
    opening + 1
}

/// A DOCTYPE without an internal subset declares no entities, and the XML
/// parser never loads its external identifier, so only `[` is refused.
fn doctype_end(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'>' => return cursor + 1,
            b'[' => {
                // The caller raises before scanning further; mirror the Ruby
                // panic-free flow by unwinding through the error channel at
                // the validate layer. Here we mark the position.
                return cursor;
            }
            b'"' | b'\'' => {
                cursor = closing(bytes, &[bytes[cursor]], cursor + 1);
            }
            _ => cursor += 1,
        }
    }
    cursor
}

fn closing(bytes: &[u8], delimiter: &[u8], start: usize) -> usize {
    match index_slice(bytes, delimiter, start) {
        Some(index) => index + delimiter.len(),
        None => bytes.len(),
    }
}

fn index_byte(bytes: &[u8], needle: u8, from: usize) -> Option<usize> {
    if from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .iter()
        .position(|byte| *byte == needle)
        .map(|position| position + from)
}

fn index_slice(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > bytes.len() || needle.is_empty() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

/// True when the policy refuses the bytes — a DOCTYPE token whose body
/// opens an internal subset. NUL bytes are stripped first (the Ruby layer
/// scans NUL-stripped bytes so NUL-padded UTF-16/32 declarations are seen).
#[must_use]
pub fn forbidden(bytes: &[u8]) -> bool {
    let filtered: Vec<u8> = bytes.iter().copied().filter(|byte| *byte != 0).collect();
    let mut cursor = 0usize;
    loop {
        let Some(opening) = index_byte(&filtered, b'<', cursor) else {
            return false;
        };
        let end = token_end(&filtered, opening);
        if filtered.len() >= opening + 10
            && &filtered[opening..opening + 9] == DOCTYPE_PREFIX
            && end < filtered.len()
            && filtered[end] == b'['
        {
            return true;
        }
        if end <= opening {
            return false;
        }
        cursor = end;
    }
}

/// Validates through the error channel.
///
/// # Errors
///
/// Returns [`Error::ForbiddenExternalReference`] with the Ruby message.
pub fn ensure_allowed(content: &str) -> Result<(), Error> {
    validate(content)
}

/// The SVG-relevant `FileMagic#detect`: `:svg` when the first 4096 bytes
/// contain `<svg`. Returns `None` for empty or unrecognizable input (the
/// EPS/PS/EMF classifications live in their own model crates).
#[must_use]
pub fn sniff_svg(bytes: &[u8]) -> Option<SvgMagic> {
    if bytes.is_empty() {
        return None;
    }
    let head = &bytes[..bytes.len().min(4096)];
    if index_slice(head, b"<svg", 0).is_some() {
        Some(SvgMagic)
    } else {
        None
    }
}

/// The SVG detection marker (the Ruby `:svg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgMagic;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_internal_subsets() {
        assert!(forbidden(
            b"<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\" [<!ENTITY a \"b\">]><svg/>"
        ));
        assert!(forbidden(b"<!DOCTYPE svg[]><svg/>"));
        assert!(forbidden(b"<!DOCTYPE svg ["));
        assert!(forbidden(
            b"<!DOCTYPE svg SYSTEM \"a>b\" [<!ENTITY a \"b\">]><svg/>"
        ));
    }

    #[test]
    fn accepts_subsetless_doctypes() {
        assert!(!forbidden(b"<!DOCTYPE svg><svg/>"));
        assert!(!forbidden(b"<!DOCTYPE svg SYSTEM 'a[b]>c'><svg/>"));
        let standard = concat!(
            "<?xml version=\"1.0\"?>",
            "<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" ",
            "\"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"3\" height=\"2\"/>"
        );
        assert!(!forbidden(standard.as_bytes()));
    }

    #[test]
    fn preserves_literal_declaration_text_in_opaque_tokens() {
        let declaration = "<!DOCTYPE svg [<!ENTITY secret SYSTEM \"file:///x\">]>";
        let safe = format!(
            "<!-- {declaration} --><?note {declaration}?><svg><![CDATA[{declaration}]]></svg>"
        );
        assert!(!forbidden(safe.as_bytes()));
    }

    #[test]
    fn continues_after_multibyte_tokens() {
        let declaration = "<!DOCTYPE svg [<!ENTITY secret SYSTEM \"file:///x\">]>";
        let source = format!("<!-- é --><![CDATA[é]]>{declaration}<svg/>");
        assert!(forbidden(source.as_bytes()));
    }

    #[test]
    fn sniffs_svg_tags() {
        assert_eq!(sniff_svg(b"<svg xmlns=\"x\"/>"), Some(SvgMagic));
        assert_eq!(sniff_svg(b""), None);
        assert_eq!(sniff_svg(b"not svg at all"), None);
    }
}
