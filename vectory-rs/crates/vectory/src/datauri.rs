//! SVG-subset port of lib/vectory/datauri.rb: parsing data URIs, decoding
//! the base64 payload, and detecting the SVG image type. The EMF and
//! PostScript mime types belong to their own model crates per the
//! model-driven law — this crate rejects them (`svg_content`), while mime
//! detection stays generic over the SVG type.

use crate::error::Error;

const SVG_MIME: &str = "image/svg+xml";

/// A parsed data URI.
#[derive(Debug, Clone)]
pub struct Datauri {
    mimetype: String,
    data: String,
}

impl Datauri {
    /// Parses a data URI (`data:<mimetype>[;charset=...];base64,<data>`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Conversion`] when the string is not a base64 data
    /// URI (the Ruby layer's message, first 30 chars quoted).
    pub fn parse(uri: &str) -> Result<Self, Error> {
        let fail = || {
            let preview: String = uri.chars().take(30).collect();
            Err(Error::Conversion(format!(
                "Could not parse datauri: '{preview}'."
            )))
        };
        let Some(rest) = uri.strip_prefix("data:") else {
            return fail();
        };
        let Some((mimetype, rest)) = rest.split_once(';') else {
            return fail();
        };
        if mimetype.is_empty() {
            return fail();
        }
        // Optional charset parameter.
        let rest = match rest.strip_prefix("charset=") {
            Some(after) => {
                let Some((_, after)) = after.split_once(';') else {
                    return fail();
                };
                after
            }
            None => rest,
        };
        let Some(data) = rest.strip_prefix("base64,") else {
            return fail();
        };
        Ok(Datauri {
            mimetype: mimetype.to_string(),
            data: data.to_string(),
        })
    }

    /// The mimetype parameter.
    #[must_use]
    pub fn mime(&self) -> &str {
        &self.mimetype
    }

    /// True when the payload decodes to an SVG.
    #[must_use]
    pub fn is_svg(&self) -> bool {
        self.mimetype == SVG_MIME
    }

    /// Decodes the payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Conversion`] on invalid base64.
    pub fn content(&self) -> Result<String, Error> {
        let bytes = strict_decode64(&self.data)
            .ok_or_else(|| Error::Conversion("invalid base64 data".to_string()))?;
        String::from_utf8(bytes)
            .map_err(|_| Error::Conversion("datauri payload is not valid UTF-8".to_string()))
    }

    /// Decodes the payload as SVG content; non-SVG mime types are rejected
    /// (their models live in their own crates).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Conversion`] on non-SVG mime types and invalid
    /// payloads.
    pub fn svg_content(&self) -> Result<String, Error> {
        if !self.is_svg() {
            return Err(Error::Conversion(format!(
                "Could not detect image type '{}'.",
                self.mimetype
            )));
        }
        self.content()
    }
}

/// The `Svg.from_content(datauri.svg_content()?)` entry point used by
/// SvgMapping.
///
/// # Errors
///
/// See [`Datauri::parse`] and [`Datauri::svg_content`].
pub fn svg_content(uri: &str) -> Result<String, Error> {
    Datauri::parse(uri)?.svg_content()
}

/// `Base64.strict_encode64`: standard alphabet, padded, no wrapping.
#[must_use]
pub fn strict_encode64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map_or(0, |b| *b as u32);
        let b2 = chunk.get(2).map_or(0, |b| *b as u32);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// `Base64.strict_decode64`: standard alphabet, padding required, no
/// whitespace tolerated.
#[must_use]
pub fn strict_decode64(input: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return Some(Vec::new());
    }
    if bytes.len() % 4 != 0 {
        return None;
    }
    let padding = bytes.iter().rev().take_while(|b| **b == b'=').count();
    if padding > 2 {
        return None;
    }
    if bytes[..bytes.len() - padding].iter().any(|b| *b == b'=') {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut triple = 0u32;
        let mut valid = 0usize;
        for &byte in chunk {
            if byte == b'=' {
                break;
            }
            triple = (triple << 6) | value(byte)?;
            valid += 1;
        }
        match valid {
            4 => {
                out.push(((triple >> 16) & 0xFF) as u8);
                out.push(((triple >> 8) & 0xFF) as u8);
                out.push((triple & 0xFF) as u8);
            }
            3 => {
                out.push(((triple >> 10) & 0xFF) as u8);
                out.push(((triple >> 2) & 0xFF) as u8);
            }
            2 => {
                out.push(((triple >> 4) & 0xFF) as u8);
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_base64() {
        let encoded = strict_encode64("hello world".as_bytes());
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");
        assert_eq!(strict_decode64(&encoded), Some("hello world".as_bytes().to_vec()));
    }

    #[test]
    fn parses_with_charset() {
        let parsed = Datauri::parse("data:image/svg+xml;charset=utf-8;base64,PHN2Zy8+").unwrap();
        assert_eq!(parsed.mime(), "image/svg+xml");
        assert_eq!(parsed.content().unwrap(), "<svg/>");
    }

    #[test]
    fn rejects_non_datauri() {
        let error = Datauri::parse("123abc").unwrap_err();
        assert!(error.to_string().contains("Could not parse datauri: '123abc'"));
    }
}
