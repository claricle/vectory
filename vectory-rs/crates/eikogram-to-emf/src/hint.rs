//! The drawing-hint comment payload encoder: the round-trip provenance a
//! drawn run carries, in the reference `emfsvg-draw:1:` JSON form.

use eikogram::Provenance;

const MAGIC: &str = "emfsvg-draw:1:";

/// The maximum hint payload the reference decoder accepts.
pub const MAX_BYTES: usize = 512;

/// Encodes a provenance payload into the comment record's data bytes:
/// the magic, then the JSON object with the record fingerprint, the kind,
/// and the carried values.
#[must_use]
pub fn encode(fingerprint: &str, provenance: &Provenance) -> Vec<u8> {
    let (kind, values) = match provenance {
        Provenance::RectText(rect) => (
            "rect",
            vec![
                json_string(&rect.x),
                json_string(&rect.y),
                json_string(&rect.width),
                json_string(&rect.height),
                json_string(&rect.fill),
            ],
        ),
        Provenance::StrokeColor(color) => (
            "stroke",
            vec![json_string(&format!(
                "#{:02X}{:02X}{:02X}",
                color.red, color.green, color.blue
            ))],
        ),
        Provenance::Jpeg => ("jpeg", Vec::new()),
    };
    let mut json = String::from("{\"record\":");
    json.push_str(&json_string(fingerprint));
    json.push_str(",\"kind\":");
    json.push_str(&json_string(kind));
    json.push_str(",\"values\":[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(value);
    }
    json.push_str("]}");
    let mut data = Vec::new();
    data.extend_from_slice(MAGIC.as_bytes());
    data.extend_from_slice(json.as_bytes());
    if data.len() > MAX_BYTES {
        // The reference decoder refuses over-length payloads; the
        // provenance is dropped rather than mis-encoded.
        return Vec::new();
    }
    data
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
