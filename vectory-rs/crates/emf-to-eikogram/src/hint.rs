//! Drawing-hint comment decoding: the reference's per-record spelling
//! provenance (round-trip contracts). Hints never alter geometry or paint;
//! they select provenance emission variants. Reference C rendering ignores
//! these comments.

/// A decoded hint: the annotated record's fingerprint, kind, and its
/// provenance values.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingHint {
    pub record_fingerprint: String,
    pub kind: String,
    pub values: Vec<String>,
}

const MAGIC: &str = "emfsvg-draw:1:";
const MAX_BYTES: usize = 512;

/// Decodes an EMR_COMMENT data payload; `None` mirrors every rejection in
/// the reference decoder (length windows, zero padding, magic, JSON shape,
/// value validation).
#[must_use]
pub fn decode(data: &[u8]) -> Option<DrawingHint> {
    // The record carries cb_data ahead of the data slice; the walker passes
    // exactly the cb_data-length slice, so the length is data.len().
    let length = data.len();
    if !(MAGIC.len()..=MAX_BYTES).contains(&length) {
        return None;
    }
    if !data.starts_with(MAGIC.as_bytes()) {
        return None;
    }
    let payload = &data[MAGIC.len()..length];
    let hint = parse_hint_json(payload)?;
    if hint.kind != "rect" && hint.kind != "stroke" && hint.kind != "jpeg" {
        return None;
    }
    match hint.kind.as_str() {
        "rect" => {
            if hint.values.len() != 5 {
                return None;
            }
            for value in hint.values.iter().take(4) {
                if !is_number_literal(value) {
                    return None;
                }
            }
            if !is_hex_color(hint.values.last()?) {
                return None;
            }
        }
        "stroke" => {
            if hint.values.len() != 1 || !is_hex_color(&hint.values[0]) {
                return None;
            }
        }
        "jpeg" => {
            if !hint.values.is_empty() {
                return None;
            }
        }
        _ => return None,
    }
    Some(DrawingHint {
        record_fingerprint: hint.record,
        kind: hint.kind,
        values: hint.values,
    })
}

#[derive(Debug)]
struct ParsedHint {
    record: String,
    kind: String,
    values: Vec<String>,
}

/// A minimal JSON object parser for the fixed hint shape
/// `{"kind": "...", "record": "<64 hex>", "values": [...]}` (keys sorted).
/// Anything unexpected returns None (mirroring JSON::ParserError).
fn parse_hint_json(payload: &[u8]) -> Option<ParsedHint> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut chars = text.chars().peekable();
    expect_char(&mut chars, '{')?;
    let mut kind: Option<String> = None;
    let mut values: Option<Vec<String>> = None;
    let mut record: Option<String> = None;
    loop {
        skip_ws(&mut chars);
        let key = parse_json_string(&mut chars)?;
        skip_ws(&mut chars);
        expect_char(&mut chars, ':')?;
        skip_ws(&mut chars);
        match key.as_str() {
            "kind" => kind = Some(parse_json_string(&mut chars)?),
            "record" => {
                let parsed = parse_json_string(&mut chars)?;
                if parsed.len() != 64 || !parsed.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
                    return None;
                }
                record = Some(parsed);
            }
            "values" => {
                expect_char(&mut chars, '[')?;
                let mut parsed: Vec<String> = Vec::new();
                loop {
                    skip_ws(&mut chars);
                    match chars.peek() {
                        Some(']') => {
                            chars.next();
                            break;
                        }
                        Some('"') => parsed.push(parse_json_string(&mut chars)?),
                        Some(',') => {
                            chars.next();
                        }
                        _ => return None,
                    }
                }
                values = Some(parsed);
            }
            _ => return None,
        }
        skip_ws(&mut chars);
        match chars.peek() {
            Some(',') => {
                chars.next();
            }
            Some('}') => {
                chars.next();
                break;
            }
            _ => return None,
        }
    }
    Some(ParsedHint {
        record: record?,
        kind: kind?,
        values: values?,
    })
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
}

fn expect_char(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    expected: char,
) -> Option<()> {
    if chars.next() == Some(expected) {
        Some(())
    } else {
        None
    }
}

fn parse_json_string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<String> {
    expect_char(chars, '"')?;
    let mut out = String::new();
    loop {
        match chars.next() {
            Some('"') => return Some(out),
            Some('\\') => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('u') => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code)?);
                }
                _ => return None,
            },
            Some(c) => out.push(c),
            None => return None,
        }
    }
}

fn is_number_literal(value: &str) -> bool {
    let body = value.strip_prefix('-').unwrap_or(value);
    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    !int_part.is_empty()
        && int_part.chars().all(|c| c.is_ascii_digit())
        && frac_part.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()))
}

fn is_hex_color(value: &str) -> bool {
    let body = value.strip_prefix('#').unwrap_or(value);
    value.starts_with('#')
        && body.len() == 6
        && body.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
}
