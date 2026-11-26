//! SVG paint and stroke parsing: the `fill`/`stroke` values, the stroke's
//! resolved pen facts, and the round-trip provenance recoveries (the null
//! sentinel, the canonical cosmetic dash), matching the reference
//! `Svg::Paint` and `Svg::Stroke`.

use crate::color::SvgColor;

/// A parsed paint: a colour (possibly null) or a pattern reference.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Null,
    Solid(SvgColor),
    /// The `url(#img-N-ref)` pattern reference (the `#img-N-ref` id).
    Pattern(String),
}

impl Paint {
    /// Parses a `fill=` value; pattern references keep their id.
    ///
    /// # Errors
    ///
    /// Fails on an unsupported colour form.
    pub fn from_fill(value: Option<&str>) -> Result<Self, String> {
        if let Some(value) = value {
            if let Some(id) = pattern_reference(value) {
                return Ok(Paint::Pattern(id));
            }
        }
        let color = SvgColor::parse(value)?;
        Ok(if matches!(color, SvgColor::Null) {
            Paint::Null
        } else {
            Paint::Solid(color)
        })
    }

    /// Parses a `stroke=` value.
    ///
    /// # Errors
    ///
    /// Fails on an unsupported colour form.
    pub fn from_stroke(value: Option<&str>) -> Result<Self, String> {
        let color = SvgColor::parse(value)?;
        Ok(if matches!(color, SvgColor::Null) {
            Paint::Null
        } else {
            Paint::Solid(color)
        })
    }

    #[must_use]
    pub fn null(&self) -> bool {
        matches!(self, Paint::Null)
    }

    /// The paint colour (null reads the null colour).
    #[must_use]
    pub fn color(&self) -> SvgColor {
        match self {
            Paint::Null => SvgColor::Null,
            Paint::Solid(color) => *color,
            Paint::Pattern(_) => SvgColor::Rgb {
                red: 0,
                green: 0,
                blue: 0,
            },
        }
    }
}

/// `#img-N-ref` (bare or `url(#…)` wrapped) → the reference id.
fn pattern_reference(value: &str) -> Option<String> {
    let bare = value.strip_prefix('#')?;
    if !is_pattern_id(bare) {
        return None;
    }
    Some(bare.to_string())
}

/// The exact pattern-reference form the reference emitter writes.
#[must_use]
pub fn is_pattern_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("img-") else {
        return false;
    };
    let Some(digits) = rest.strip_suffix("-ref") else {
        return false;
    };
    !digits.is_empty() && !digits.starts_with('0') && digits.chars().all(|c| c.is_ascii_digit())
}

/// A parsed stroke: the resolved pen facts of one element's stroke
/// attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub paint: Paint,
    /// The parsed width value (the leading-number prefix).
    pub width: f64,
    pub dash_array: Option<Vec<f64>>,
    pub line_cap: String,
    pub line_join: String,
    pub miter_limit: f64,
    /// The raw width attribute text (the provenance forms read it).
    pub raw_width: String,
    pub line_cap_explicit: bool,
    pub line_join_explicit: bool,
    /// True unless the fill was explicitly null (the null-pen fallback
    /// governs the sentinel form).
    pub null_pen_fallback: bool,
    /// The brush the run selects when the pen is null (the brush-fallback
    /// provenance form).
    pub fallback_brush: Option<Paint>,
    /// The recovered canonical cosmetic dash: the stored width and the pen
    /// style it reproduces.
    pub canonical_cosmetic_dash: Option<(f64, u32)>,
}

impl Stroke {
    /// Parses stroke attributes alongside the fill paint.
    ///
    /// # Errors
    ///
    /// Fails on an unsupported colour form.
    pub fn parse(
        stroke: Option<&str>,
        width: Option<&str>,
        dash_array: Option<&str>,
        line_cap: Option<&str>,
        line_join: Option<&str>,
        miter_limit: Option<&str>,
        fill: &Paint,
    ) -> Result<Self, String> {
        let paint = Paint::from_stroke(stroke)?;
        Ok(Stroke {
            paint,
            width: parse_width(width),
            dash_array: parse_dash_array(dash_array),
            line_cap: line_cap.unwrap_or("round").to_string(),
            line_join: line_join.unwrap_or("round").to_string(),
            miter_limit: parse_miter_limit(miter_limit),
            raw_width: width.unwrap_or("").to_string(),
            line_cap_explicit: line_cap.is_some(),
            line_join_explicit: line_join.is_some(),
            null_pen_fallback: !fill.null(),
            fallback_brush: None,
            canonical_cosmetic_dash: None,
        })
    }

    /// The exact unfilled-Polyline spelling the legacy cosmetic pens emit;
    /// recovers the pen style and stored width only for that exact source
    /// (the attribute sequence the renderer writes; the tag's whitespace
    /// carries nothing beyond it).
    pub fn with_canonical_cosmetic_dash(
        &mut self,
        d: Option<&str>,
        stroke: Option<&str>,
        dasharray: Option<&str>,
        linecap: Option<&str>,
        linejoin: Option<&str>,
        fill: Option<&str>,
    ) {
        if !matches!(self.raw_width.as_str(), "1.0000" | "1px")
            || self.paint.null()
            || self.dash_array.is_none()
        {
            return;
        }
        let (Some(d), Some(stroke), Some(dasharray), Some(fill)) = (d, stroke, dasharray, fill)
        else {
            return;
        };
        let Some(dash) = self.dash_array.as_ref() else {
            return;
        };
        if linecap != Some("round") || linejoin != Some("round") {
            return;
        }
        if fill != "none" || dasharray != attr_text(&self.dash_array) {
            return;
        }
        // The data shape: `M x,y( L x,y)+ ` with an uppercase hex stroke.
        let _ = stroke;
        if !crate::provenance::open_line_run(d, 1) {
            return;
        }
        for (style, pattern) in COSMETIC_DASH_PATTERNS {
            let stored = dash[0] / pattern[0];
            if stored < 1.0 || stored != stored.trunc() {
                continue;
            }
            let scaled: Vec<f64> = pattern.iter().map(|length| length * stored).collect();
            if dash == &scaled {
                self.canonical_cosmetic_dash = Some((stored, style));
                return;
            }
        }
    }

    /// The `stroke-width="1px"` sentinel form: a null pen whose fallback
    /// brush carries the rendering.
    #[must_use]
    pub fn null_pen_sentinel(&self) -> bool {
        self.raw_width == "1px"
            && self.null_pen_fallback
            && !self.line_cap_explicit
            && !self.line_join_explicit
    }

    /// A cosmetic pixel width: a real 1px pen (non-null, not the sentinel).
    #[must_use]
    pub fn cosmetic_pixel_width(&self) -> bool {
        self.raw_width == "1px" && !self.paint.null() && !self.null_pen_sentinel()
    }

    /// The pen style bitmask (dash family, type bit, end cap, line join).
    #[must_use]
    pub fn pen_style(&self) -> u32 {
        if let Some((_, style)) = self.canonical_cosmetic_dash {
            return style;
        }
        let base: u32 = if self.paint.null() || self.null_pen_sentinel() {
            5 // PS_NULL
        } else {
            self.dash_style()
        };
        let type_bit: u32 =
            if (self.width > 1.0 || self.explicit_unitless_one_width()) && !self.paint.null() {
                0x0001_0000
            } else {
                0
            };
        base | type_bit | end_cap_bits(&self.line_cap) | line_join_bits(&self.line_join)
    }

    /// The discrete dash style the first dash length maps to.
    #[must_use]
    pub fn dash_style(&self) -> u32 {
        let Some(dash) = &self.dash_array else {
            return 0;
        };
        let Some(first) = dash.first() else {
            return 0;
        };
        if *first == 0.0 {
            return 0;
        }
        if *first < self.width * 2.0 {
            2 // PS_DOT
        } else {
            1 // PS_DASH
        }
    }

    /// An explicit unitless width of exactly one.
    #[must_use]
    pub fn explicit_unitless_one_width(&self) -> bool {
        let trimmed = self.raw_width.trim();
        if trimmed.is_empty() {
            return false;
        }
        // `\A\s*1(?:\.0+)?\s*\z`
        let body = trimmed.trim_matches(|c: char| c.is_whitespace());
        let Some(rest) = body.strip_prefix('1') else {
            return false;
        };
        let Some(frac) = rest.strip_prefix('.') else {
            return rest.is_empty();
        };
        !frac.is_empty() && frac.chars().all(|c| c == '0')
    }
}

const COSMETIC_DASH_PATTERNS: [(u32, [f64; 4]); 4] = [
    (1, [5.0, 5.0, 0.0, 0.0]),
    (2, [1.0, 1.0, 0.0, 0.0]),
    (3, [5.0, 5.0, 1.0, 5.0]),
    (4, [5.0, 5.0, 1.0, 1.0]),
];

/// The `<path d="M x,y L … " stroke="#HEX">` prefix the canonical cosmetic
/// dash source carries: returns `(d, color)`.
/// The dash-array attribute text: the lengths in the reference's `%.4f`
/// form, comma-joined.
#[must_use]
pub fn attr_text(dash: &Option<Vec<f64>>) -> String {
    dash.as_ref()
        .map(|values| {
            values
                .iter()
                .map(|value| crate::number::fmt4(*value))
                .collect::<Vec<String>>()
                .join(",")
        })
        .unwrap_or_default()
}

#[allow(dead_code)]
fn cosmetic_dash_source(source: &str) -> Option<(String, String)> {
    // `\A<path d="(M x,y(?: L x,y)+ )"` then ` stroke="(#[0-9A-F]{6})"`.
    if !source.starts_with("<path d=\"M ") {
        return None;
    }
    let data_start = "<path d=\"".len();
    let rest = &source[data_start..];
    let quote = rest.find('"')?;
    let data = &rest[..quote];
    // `M x,y( L x,y)+ ` — at least one line and a trailing space.
    let mut segments = data.trim_end().split(' ');
    if segments.next() != Some("M") {
        return None;
    }
    let mut count = 0;
    while let Some(part) = segments.next() {
        if part != "L" {
            return None;
        }
        segments.next()?;
        count += 1;
    }
    if count == 0 || !data.ends_with(' ') {
        return None;
    }
    let after = &rest[quote + 1..];
    let color = after.strip_prefix(" stroke=\"#")?;
    if color.len() < 6 {
        return None;
    }
    let hex = &color[..6];
    if !hex
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
    {
        return None;
    }
    Some((data.to_string(), format!("#{hex}")))
}

fn end_cap_bits(cap: &str) -> u32 {
    match cap {
        "butt" => 0x0200,
        "square" => 0x0100,
        _ => 0x0000,
    }
}

fn line_join_bits(join: &str) -> u32 {
    match join {
        "bevel" => 0x1000,
        "miter" => 0x2000,
        _ => 0x0000,
    }
}

fn parse_width(value: Option<&str>) -> f64 {
    let Some(value) = value else {
        return 1.0;
    };
    if value.trim().is_empty() {
        return 1.0;
    }
    crate::attributes::parse_float(Some(value), 1.0)
}

fn parse_dash_array(value: Option<&str>) -> Option<Vec<f64>> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed == "none" || trimmed.is_empty() {
        return None;
    }
    let parts: Vec<f64> = trimmed
        .split([' ', '\t', '\n', '\r', ','])
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn parse_miter_limit(value: Option<&str>) -> f64 {
    let Some(value) = value else {
        return 10.0;
    };
    if value.trim().is_empty() {
        return 10.0;
    }
    value.trim().parse::<f64>().unwrap_or(10.0)
}
