//! SVG attribute parsing helpers: numbers and point lists, matching the
//! reference `AttributeParser` exactly (leading-number prefix parse with
//! default fallback).

/// Parses a leading float, defaulting when absent or unparseable.
#[must_use]
pub fn parse_float(value: Option<&str>, default: f64) -> f64 {
    let Some(value) = value else {
        return default;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return default;
    }
    leading_float(trimmed).unwrap_or(default)
}

/// The leading `-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?` prefix of a value.
fn leading_float(value: &str) -> Option<f64> {
    let bytes = value.as_bytes();
    let mut end = 0usize;
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end == digits_start {
        return None;
    }
    // Optional exponent.
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut exp_end = end + 1;
        if exp_end < bytes.len() && (bytes[exp_end] == b'-' || bytes[exp_end] == b'+') {
            exp_end += 1;
        }
        let exp_digits = exp_end;
        while exp_end < bytes.len() && bytes[exp_end].is_ascii_digit() {
            exp_end += 1;
        }
        if exp_end > exp_digits {
            end = exp_end;
        }
    }
    value[..end].parse::<f64>().ok()
}

/// Parses a point list like `1,2 3,4` into `[x, y]` pairs (the reference
/// splits on whitespace and commas and slices pairs).
#[must_use]
pub fn parse_points(value: Option<&str>) -> Vec<(f64, f64)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let numbers: Vec<f64> = trimmed
        .split([' ', '\t', '\n', '\r', ','])
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect();
    numbers
        .chunks(2)
        .map(|pair| (pair[0], *pair.get(1).unwrap_or(&0.0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_match_reference_prefix_parse() {
        assert_eq!(parse_float(Some("12.5px"), 0.0), 12.5);
        assert_eq!(parse_float(Some(" -3 "), 0.0), -3.0);
        assert_eq!(parse_float(Some(""), 7.0), 7.0);
        assert_eq!(parse_float(None, 7.0), 7.0);
        assert_eq!(parse_float(Some("abc"), 4.0), 4.0);
    }

    #[test]
    fn points_split_pairs() {
        assert_eq!(parse_points(Some("1,2 3,4")), vec![(1.0, 2.0), (3.0, 4.0)]);
        assert_eq!(parse_points(None), Vec::new());
    }
}
