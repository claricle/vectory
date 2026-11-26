//! Provenance recovery: the renderer's exact SVG spellings that carry
//! record-family facts back through the round trip. Every check is an
//! exact template match against the raw source tag and the attribute
//! values, mirroring the reference `Svg::Elements` canonical forms.

use crate::path_data::Command;

fn attrs_names(node: &vectory::Element) -> Vec<&str> {
    node.attributes.iter().map(|a| a.name.as_str()).collect()
}

/// True when a value is a four-decimal number.
#[must_use]
pub fn is_number(value: &str) -> bool {
    let body = value.strip_prefix('-').unwrap_or(value);
    let Some((integers, decimals)) = body.split_once('.') else {
        return false;
    };
    !integers.is_empty()
        && integers.chars().all(|c| c.is_ascii_digit())
        && decimals.len() == 4
        && decimals.chars().all(|c| c.is_ascii_digit())
}

/// True when a value matches `#[0-9A-Fa-f]{6}` (either hex case).
#[must_use]
fn is_hex6_either(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// True when a value matches `#[0-9A-F]{6}` (uppercase only).
#[must_use]
fn is_hex6_upper(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'A'..='F'))
}

/// True when a value matches `#[0-9a-f]{6}` (lowercase only).
#[must_use]
fn is_hex6_lower(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
}

/// The stroke-width provenance form: a four-decimal number or `1px`.
#[must_use]
fn is_stroke_width(value: &str) -> bool {
    value == "1px" || is_number(value)
}

/// True when the value is `url(#clip-N)`.
#[must_use]
fn is_clip_reference(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("url(#clip-") else {
        return false;
    };
    let Some(digits) = rest.strip_suffix(')') else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// The path's line-record family from its spelling.
#[must_use]
pub fn line_record_kind(
    node: &vectory::Element,
    source_tag: Option<&str>,
) -> Option<crate::elements::LineKind> {
    if canonical_polyline(node, source_tag) {
        return Some(crate::elements::LineKind::Polyline);
    }
    let d = node.attribute("d")?;
    if !two_points_open(d) {
        return None;
    }
    stroked_path(node, source_tag, false, false).then_some(crate::elements::LineKind::PolylineTo)
}

/// `M x,y( L x,y){2,} ` — three or more points, trailing space.
fn two_points_open(d: &str) -> bool {
    // The PolylineTo spelling: M plus two or more L points.
    contour_points(d, 2, false, ' ')
}

/// The path's record family from its spelling.
#[must_use]
pub fn path_record_kind(
    node: &vectory::Element,
    source_tag: Option<&str>,
) -> Option<crate::elements::PathKind> {
    use crate::elements::PathKind;
    if let Some(kind) = canonical_arc_kind(node, source_tag) {
        return Some(kind);
    }
    if canonical_pending_path(node, source_tag) {
        return Some(PathKind::EndPath);
    }
    if canonical_filled_curve(node, source_tag) || canonical_filled_contours(node, source_tag) {
        return Some(PathKind::FillPath);
    }
    if canonical_bitblt(node, source_tag) {
        return Some(PathKind::BitBlt);
    }
    if canonical_poly_polygon(node, source_tag) {
        return Some(PathKind::PolyPolygon);
    }
    if canonical_open_painted_path(node, source_tag) {
        return Some(PathKind::StrokeAndFillPath);
    }
    if canonical_stroked_curve(node, source_tag) {
        return Some(PathKind::PolyBezierTo);
    }
    if canonical_cubic_figure(node, source_tag) || canonical_orphan_or_empty_path(node, source_tag)
    {
        return Some(PathKind::StrokePath);
    }
    None
}

/// A NULL pen uses its solid brush to draw open polyline strokes; the
/// renderer writes `fill=none` even though that brush is set.
#[must_use]
pub fn brush_fallback(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !open_line_run(d, 1) {
        return false;
    }
    let Some(stroke) = node.attribute("stroke") else {
        return false;
    };
    if !is_hex6_upper(stroke) {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    source == format!("<path d=\"{d}\" stroke-width=\"1px\" stroke=\"{stroke}\" fill=\"none\" />")
}

/// An EndPath without a painting record: `fill="none" stroke="none"`.
fn canonical_pending_path(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !open_mixed_run(d, false, false) {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    source == format!("<path d=\"{d}\" fill=\"none\" stroke=\"none\" />")
}

/// A pure cubic FillPath omits PolyBezier's extra current-position move.
fn canonical_filled_curve(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !curve_only_run(d) {
        return false;
    }
    let Some(fill) = node.attribute("fill") else {
        return false;
    };
    if !is_hex6_upper(fill) {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    source == format!("<path d=\"{d}\"  fill=\"{fill}\" stroke=\"none\" />")
}

/// FillPath retains every closed contour with a fill-first tag and no pen
/// attributes.
fn canonical_filled_contours(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !closed_contours(d) {
        return false;
    }
    let Some(fill) = node.attribute("fill") else {
        return false;
    };
    if !is_hex6_upper(fill) {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    source == format!("<path d=\"{d}\"  fill=\"{fill}\" stroke=\"none\" />")
}

/// Path painting implicitly closes only the fill, retaining open strokes;
/// the fill-first output distinguishes it.
fn canonical_open_painted_path(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !open_mixed_run(d, false, false) {
        return false;
    }
    fill_first_style(node, source_tag)
}

/// PolyBezier includes a separate current-position move; PolyBezierTo
/// emits only its existing current position.
fn canonical_stroked_curve(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !curve_only_run(d) {
        return false;
    }
    stroked_path(node, source_tag, true, false)
}

/// StrokePath figures have one initial move and the ordinary fill gap;
/// cubics distinguish these from polyline runs.
fn canonical_cubic_figure(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !d.contains(" C ") {
        return false;
    }
    if !open_mixed_run(d, true, true) {
        return false;
    }
    stroked_path(node, source_tag, false, false)
}

/// A stroked EMF path can start with LineTo or contain no figures at all;
/// only the renderer's exact spelling carries that provenance.
fn canonical_orphan_or_empty_path(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let data = node.attribute("d").unwrap_or("");
    let valid = data.is_empty() || leading_line_to(data);
    valid && stroked_path(node, source_tag, false, false)
}

/// Zero-source BitBlt emits a style-first rectangle without a trailing
/// space after `Z`.
fn canonical_bitblt(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(style) = node.attribute("style") else {
        return false;
    };
    let Some(value) = style.strip_prefix("fill:") else {
        return false;
    };
    if !is_hex6_lower(value) {
        return false;
    }
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !bitblt_rectangle(d) {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    source == format!("<path style=\"{style}\" d=\"{d}\" />")
}

/// PolylineTo starts at the current position with an extra space before
/// fill; a single line shares that spelling with LineTo.
fn canonical_polyline(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !two_point_line(d) {
        return false;
    }
    stroked_path(node, source_tag, false, false)
}

/// The renderer's arc spellings: `M x,y M x,y A rx,ry 0 s s x,y [closure]`.
#[must_use]
pub fn canonical_arc_kind(
    node: &vectory::Element,
    source_tag: Option<&str>,
) -> Option<crate::elements::PathKind> {
    use crate::elements::PathKind;
    let d = node.attribute("d")?;
    let parsed = parse_arc_spelling(d)?;
    let (_sweep, closure) = parsed;
    let kind = match closure {
        None => PathKind::Arc,
        Some(line) => {
            if line {
                PathKind::Pie
            } else {
                PathKind::Chord
            }
        }
    };
    stroked_path(node, source_tag, true, kind != PathKind::Arc).then_some(kind)
}

/// Parses the arc spelling; returns (sweep, closure) where the closure is
/// `Some(true)` for the `L … Z` pie form and `Some(false)` for bare `Z`.
fn parse_arc_spelling(d: &str) -> Option<(bool, Option<bool>)> {
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    // M x,y M x,y A rx,ry 0 sweep sweep x,y [closure...]
    if tokens.len() < 13 || tokens[0] != "M" || tokens[3] != "M" || tokens[6] != "A" {
        return None;
    }
    if !tokens[7].contains(',') || !tokens[8].contains(',') {
        return None;
    }
    if tokens[9] != "0" {
        return None;
    }
    let sweep = match tokens[10] {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    if tokens[11] != tokens[10] {
        return None;
    }
    if !tokens[12].contains(',') {
        return None;
    }
    let mut cursor = 13;
    let mut closure = None;
    if tokens.len() > cursor {
        if tokens[cursor] == "L" {
            if cursor + 3 > tokens.len() || !tokens[cursor + 1].contains(',') {
                return None;
            }
            if tokens.get(cursor + 2) != Some(&"Z") {
                return None;
            }
            closure = Some(true);
            cursor += 3;
        } else if tokens[cursor] == "Z" {
            closure = Some(false);
            cursor += 1;
        } else {
            return None;
        }
    }
    if cursor != tokens.len() {
        return None;
    }
    // Validate the numeric shapes.
    for coordinate in [
        tokens[1], tokens[2], tokens[4], tokens[5], tokens[7], tokens[8], tokens[12],
    ] {
        let (x, y) = coordinate.split_once(',')?;
        if !is_number(x) || !is_number(y) {
            return None;
        }
    }
    let (arc_rx, _) = tokens[7].split_once(',')?;
    if !is_number(arc_rx) {
        return None;
    }
    Some((sweep, closure))
}

/// The shared stroked-path template check: the exact attribute sequence
/// the renderer writes (quirk whitespace carries nothing beyond it).
#[allow(clippy::too_many_arguments)]
fn stroked_path(
    node: &vectory::Element,
    source_tag: Option<&str>,
    fill_gap: bool,
    closed: bool,
) -> bool {
    let Some(stroke) = node.attribute("stroke") else {
        return false;
    };
    if !is_hex6_either(stroke) {
        return false;
    }
    let Some(width) = node.attribute("stroke-width") else {
        return false;
    };
    if !is_stroke_width(width) {
        return false;
    }
    let Some(cap) = node.attribute("stroke-linecap") else {
        return false;
    };
    if !matches!(cap, "round" | "square" | "butt") {
        return false;
    }
    let Some(join) = node.attribute("stroke-linejoin") else {
        return false;
    };
    if !matches!(join, "round" | "bevel" | "miter") {
        return false;
    }
    let Some(fill) = node.attribute("fill") else {
        return false;
    };
    let fill_ok = if closed {
        is_hex6_either(fill)
    } else {
        fill == "none"
    };
    if !fill_ok {
        return false;
    }
    let d = node.attribute("d").unwrap_or("");
    let dash = node.attribute("stroke-dasharray");
    let miter = node.attribute("stroke-miterlimit");
    let clip = node.attribute("clip-path");
    if let Some(clip) = clip {
        if !is_clip_reference(clip) {
            return false;
        }
    }
    // The reference's exact start-tag template; the whitespace between the
    // attributes carries the fill-gap and closed distinctions.
    let Some(source) = source_tag else {
        return false;
    };
    let mut expected = String::from("<path ");
    if let Some(clip) = clip {
        expected.push_str(&format!(" clip-path=\"{clip}\" "));
    }
    expected.push_str(&format!(
        "d=\"{d}\" stroke=\"{stroke}\" stroke-width=\"{width}\" "
    ));
    if let Some(dash) = dash {
        expected.push_str(&format!("stroke-dasharray=\"{dash}\" "));
    }
    expected.push_str(&format!(
        " stroke-linecap=\"{cap}\"  stroke-linejoin=\"{join}\" "
    ));
    if let Some(miter) = miter {
        expected.push_str(&format!(" stroke-miterlimit=\"{miter}\" "));
    }
    if fill_gap {
        expected.push(' ');
    }
    expected.push_str(&format!("fill=\"{fill}\""));
    if closed {
        expected.push(' ');
    }
    expected.push_str(" />");
    source == expected
}

/// PolyPolygon's fill-first start tag with the trailing space after `Z`.
fn canonical_poly_polygon(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(d) = node.attribute("d") else {
        return false;
    };
    if !closed_single_contour(d) {
        return false;
    }
    fill_first_style(node, source_tag)
}

/// The fill-first template: the exact attribute sequence `d`, fill,
/// stroke, width, extras (the renderer's fill-first start tag).
fn fill_first_style(node: &vectory::Element, source_tag: Option<&str>) -> bool {
    let Some(fill) = node.attribute("fill") else {
        return false;
    };
    if !is_hex6_upper(fill) {
        return false;
    }
    let Some(stroke) = node.attribute("stroke") else {
        return false;
    };
    if !is_hex6_upper(stroke) {
        return false;
    }
    let Some(width) = node.attribute("stroke-width") else {
        return false;
    };
    if !is_stroke_width(width) {
        return false;
    }
    let Some(cap) = node.attribute("stroke-linecap") else {
        return false;
    };
    if !matches!(cap, "round" | "square" | "butt") {
        return false;
    }
    let Some(join) = node.attribute("stroke-linejoin") else {
        return false;
    };
    if !matches!(join, "round" | "bevel" | "miter") {
        return false;
    }
    if node.attribute("stroke-dasharray").is_some() {
        return false;
    }
    let d = node.attribute("d").unwrap_or("");
    let miter = node.attribute("stroke-miterlimit");
    if miter.is_some() && join != "miter" {
        return false;
    }
    let Some(source) = source_tag else {
        return false;
    };
    let mut expected = format!("<path d=\"{d}\"  fill=\"{fill}\" stroke=\"{stroke}\" ");
    expected.push_str(&format!(
        "stroke-width=\"{width}\"  stroke-linecap=\"{cap}\" "
    ));
    expected.push_str(&format!(" stroke-linejoin=\"{join}\" "));
    if let Some(miter) = miter {
        expected.push_str(&format!(" stroke-miterlimit=\"{miter}\" "));
    }
    expected.push_str("/>");
    source == expected
}

/// The canonical matrix group: a lone `transform` attribute holding
/// `matrix(a b c d e f)` with six four-decimal numbers.
#[must_use]
pub fn canonical_matrix_node(node: &vectory::Element) -> bool {
    let names: Vec<&str> = node.attributes.iter().map(|a| a.name.as_str()).collect();
    if names != ["transform"] {
        return false;
    }
    let Some(value) = node.attribute("transform") else {
        return false;
    };
    let Some(inner) = value.strip_prefix("matrix(") else {
        return false;
    };
    let Some(inner) = inner.strip_suffix(')') else {
        return false;
    };
    let parts: Vec<&str> = inner.split(' ').filter(|p| !p.is_empty()).collect();
    parts.len() == 6 && parts.iter().all(|part| is_number(part))
}

// --- geometry shape recognizers (the exact renderer spellings) ---

type Coordinate = (bool, bool);

fn coordinate(token: &str) -> Option<Coordinate> {
    let (x, y) = token.split_once(',')?;
    Some((is_number(x), is_number(y)))
}

/// `M p( L p){n,} end` where `end` is a trailing space (open) or nothing.
#[allow(clippy::needless_pass_by_value)]
fn contour_points(d: &str, min_lines: usize, require_close: bool, end: char) -> bool {
    if !d.starts_with("M ") {
        return false;
    }
    let body = &d[2..];
    let tokens: Vec<&str> = body.split(' ').filter(|t| !t.is_empty()).collect();
    let Some(first) = tokens.first() else {
        return false;
    };
    if coordinate(first) != Some((true, true)) {
        return false;
    }
    let mut index = 1;
    let mut lines = 0;
    while index + 1 < tokens.len() && tokens[index] == "L" {
        if coordinate(tokens[index + 1]) != Some((true, true)) {
            return false;
        }
        lines += 1;
        index += 2;
    }
    if lines < min_lines {
        return false;
    }
    if index != tokens.len() {
        return false;
    }
    if require_close {
        return d.ends_with("Z");
    }
    end == ' ' && d.ends_with(' ') || end == '\0' && !d.ends_with(' ')
}

/// `M x,y( L x,y)+ ` — at least one line, trailing space.
#[must_use]
pub fn open_line_run(d: &str, min_lines: usize) -> bool {
    contour_points(d, min_lines, false, ' ')
}

/// `M x,y L x,y ` — exactly one line, trailing space.
fn two_point_line(d: &str) -> bool {
    contour_points(d, 1, false, ' ') && d.matches('L').count() == 1
}

/// `M p( L p| C p p p)+ end`.
fn open_mixed_run(d: &str, allow_close: bool, _unused: bool) -> bool {
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() < 5 || tokens[0] != "M" {
        return false;
    }
    if coordinate(tokens[1]) != Some((true, true)) {
        return false;
    }
    let mut index = 2;
    while index < tokens.len() {
        match tokens[index] {
            "L" => {
                if index + 1 >= tokens.len() || coordinate(tokens[index + 1]) != Some((true, true))
                {
                    return false;
                }
                index += 2;
            }
            "C" => {
                if index + 3 >= tokens.len() {
                    return false;
                }
                for offset in 1..=3 {
                    if coordinate(tokens[index + offset]) != Some((true, true)) {
                        return false;
                    }
                }
                index += 4;
            }
            "Z" if allow_close => {
                index += 1;
                if index != tokens.len() {
                    return false;
                }
                return d.ends_with("Z ");
            }
            _ => return false,
        }
    }
    // The open forms end with a trailing space.
    d.ends_with(' ') && !d.ends_with("  ")
}

/// `M p( C p p p)+ ` — a cubic-only run with trailing space.
fn curve_only_run(d: &str) -> bool {
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() < 11 || tokens[0] != "M" {
        return false;
    }
    if coordinate(tokens[1]) != Some((true, true)) {
        return false;
    }
    let mut index = 2;
    while index < tokens.len() {
        if tokens[index] != "C" {
            return false;
        }
        if index + 3 >= tokens.len() {
            return false;
        }
        for offset in 1..=3 {
            if coordinate(tokens[index + offset]) != Some((true, true)) {
                return false;
            }
        }
        index += 4;
    }
    d.ends_with(' ')
}

/// `(M p( L p| C p p p)+ Z )+ ` — one or more closed contours, then a
/// final `Z` with no trailing space (the filled-contours form).
fn closed_contours(d: &str) -> bool {
    if !d.starts_with("M ") || !d.ends_with("Z") {
        return false;
    }
    let body = &d[..d.len() - 1];
    let contours: Vec<&str> = body.split("Z ").filter(|c| !c.is_empty()).collect();
    !contours.is_empty()
        && contours
            .iter()
            .all(|contour| open_mixed_run(&format!("{contour}Z "), true, false))
}

/// `M p( L p)+ Z ` — a single closed contour with the trailing space.
fn closed_single_contour(d: &str) -> bool {
    if !d.starts_with("M ") || !d.ends_with("Z ") {
        return false;
    }
    let inner = d.strip_suffix("Z ").expect("checked above");
    let tokens: Vec<&str> = inner.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() < 3 || tokens[0] != "M" {
        return false;
    }
    if coordinate(tokens[1]) != Some((true, true)) {
        return false;
    }
    let mut index = 2;
    let mut lines = 0;
    while index + 1 < tokens.len() && tokens[index] == "L" {
        if coordinate(tokens[index + 1]) != Some((true, true)) {
            return false;
        }
        lines += 1;
        index += 2;
    }
    lines >= 1 && index == tokens.len() && d.matches('Z').count() == 1
}

/// `L p( [ML] p)* ` — the orphan-path form (leading LineTo).
fn leading_line_to(d: &str) -> bool {
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() < 2 || tokens[0] != "L" {
        return false;
    }
    if coordinate(tokens[1]) != Some((true, true)) {
        return false;
    }
    let mut index = 2;
    while index < tokens.len() {
        if tokens[index] != "M" && tokens[index] != "L" {
            return false;
        }
        if index + 1 >= tokens.len() || coordinate(tokens[index + 1]) != Some((true, true)) {
            return false;
        }
        index += 2;
    }
    d.ends_with(' ')
}

/// The BitBlt rectangle: `M x,y L xr,y L xr,yb L x,yb Z` (no trailing
/// space) with the shared corners.
fn bitblt_rectangle(d: &str) -> bool {
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() != 9
        || tokens[0] != "M"
        || tokens[3] != "L"
        || tokens[6] != "L"
        || tokens[8] != "Z"
    {
        return false;
    }
    let (Some((x, y)), Some((right, y2)), Some((right2, bottom)), Some((x2, bottom2))) = (
        coordinate(tokens[1]),
        coordinate(tokens[4]),
        coordinate(tokens[7]),
        coordinate(if tokens[9].is_empty() { "" } else { tokens[8] }),
    ) else {
        return false;
    };
    let _ = (x, y, right, y2, right2, bottom, x2, bottom2);
    x && y && right && y2 && right2 && bottom && x2 && bottom2
}

/// The canonical selected clip path: the exact renderer clip spelling
/// (a lone `d` attribute, the exact render tag, the shape shapes).
#[must_use]
pub fn canonical_selected_path(
    node: &vectory::Element,
    child_node: Option<&vectory::Element>,
) -> Option<String> {
    let id = node.attribute("id")?;
    if !is_clip_id(id) || node.attributes.len() != 1 {
        return None;
    }
    let path = child_node?;
    if path.local_name() != "path" || attrs_names(path) != ["d"] {
        return None;
    }
    let d = path.attribute("d")?;
    if !selected_path_shape(d) {
        return None;
    }
    d.strip_suffix('Z').map(str::to_string)
}

/// The canonical unused rectangle: the renderer's five-point closed path.
#[must_use]
pub fn canonical_unused_rectangle(
    node: &vectory::Element,
    child_node: Option<&vectory::Element>,
) -> Option<[f64; 4]> {
    let id = node.attribute("id")?;
    if !is_clip_id(id) || node.attributes.len() != 1 {
        return None;
    }
    let path = child_node?;
    if path.local_name() != "path" || attrs_names(path) != ["d"] {
        return None;
    }
    let d = path.attribute("d")?;
    unused_rectangle_points(d)
}

fn unused_rectangle_points(d: &str) -> Option<[f64; 4]> {
    // The reference's five-point closed rectangle: M x,y L r,t L r,b L x,b
    // L x,t Z Z (twelve tokens; the fifth contour point repeats the
    // first).
    let tokens: Vec<&str> = d.split(' ').filter(|t| !t.is_empty()).collect();
    if tokens.len() != 12 || tokens[0] != "M" {
        return None;
    }
    for index in [2, 4, 6, 8] {
        if tokens.get(index) != Some(&"L") {
            return None;
        }
    }
    if tokens.get(10) != Some(&"Z") || tokens.get(11) != Some(&"Z") || tokens.get(12).is_some() {
        return None;
    }
    let mut values: Vec<f64> = Vec::with_capacity(10);
    for index in [1, 3, 5, 7, 9] {
        let token = tokens.get(index)?;
        let (x, y) = token.split_once(',')?;
        if !is_number(x) || !is_number(y) {
            return None;
        }
        values.push(x.parse::<f64>().ok()?);
        values.push(y.parse::<f64>().ok()?);
    }
    // Corner consistency: p2=(r,t), p3=(r,b), p4=(l,b), p5=(l,t).
    let (left, top, right, bottom) = (values[0], values[1], values[2], values[5]);
    let same = [
        [values[2], values[3], right, top],
        [values[4], values[5], right, bottom],
        [values[6], values[7], left, bottom],
        [values[8], values[9], left, top],
    ];
    if !same
        .iter()
        .all(|pair| pair[0] == pair[2] && pair[1] == pair[3])
    {
        return None;
    }
    if left > right || top > bottom {
        return None;
    }
    Some([left, top, right, bottom])
}

fn selected_path_shape(d: &str) -> bool {
    // The selected clip shape: open mixed run + trailing `Z` appended, or
    // two or more closed contours then a final `Z`.
    if d.starts_with("M ") && d.ends_with('Z') {
        let inner = &d[..d.len() - 1];
        if inner.ends_with(' ') && open_mixed_run(inner, false, false) {
            return true;
        }
        if closed_contours_multi(inner) {
            return true;
        }
    }
    false
}

fn closed_contours_multi(d: &str) -> bool {
    // `(M p( L p| C p p p)+ Z ){2,}Z`
    let mut rest = d;
    let mut count = 0;
    while let Some(position) = rest.find("Z ") {
        let contour = &rest[..position + 1];
        if !open_mixed_run(&format!("{contour} "), true, false) {
            return false;
        }
        count += 1;
        rest = &rest[position + 2..];
    }
    count >= 2 && rest == "Z"
}

fn is_clip_id(id: &str) -> bool {
    let Some(digits) = id.strip_prefix("clip-") else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// The canonical defs pattern (unused or selected): the exact image +
/// pattern + use provenance the DIB-brush renderer emits.
#[must_use]
pub fn canonical_pattern(
    node: &vectory::Element,
    root: &vectory::Element,
    selected: bool,
) -> Option<crate::elements::CanonicalPattern> {
    let children: Vec<&vectory::Element> = node.child_elements().collect();
    if children.len() != 2 || !node.attributes.is_empty() {
        return None;
    }
    let image = children[0];
    let pattern = children[1];
    if image.local_name() != "image" || pattern.local_name() != "pattern" {
        return None;
    }
    let id = image.attribute("id")?;
    let width = image.attribute("width")?;
    let height = image.attribute("height")?;
    if !is_image_id(id)
        || !positive_integer(width)
        || !positive_integer(height)
        || !canonical_image_attributes(image, id, width, height)
    {
        return None;
    }
    let expected_pairs: Vec<(String, String)> = vec![
        ("id".to_string(), format!("{id}-ref")),
        ("x".to_string(), "0".to_string()),
        ("y".to_string(), "0".to_string()),
        ("width".to_string(), width.to_string()),
        ("height".to_string(), height.to_string()),
        ("patternUnits".to_string(), "userSpaceOnUse".to_string()),
    ];
    if attribute_pairs(pattern) != expected_pairs {
        return None;
    }
    let uses: Vec<&vectory::Element> = pattern.child_elements().collect();
    if uses.len() != 1 || uses[0].local_name() != "use" {
        return None;
    }
    let use_element = uses[0];
    let use_pairs: Vec<(String, String)> = vec![
        ("id".to_string(), format!("{id}-ign")),
        ("xlink:href".to_string(), format!("#{id}")),
    ];
    if attribute_pairs(use_element) != use_pairs || use_element.child_elements().next().is_some() {
        return None;
    }
    // The reference check walks the whole document: a selected pattern
    // must be referenced; an unused one must not be (stylesheets count as
    // references).
    if !pattern_reference_check(root, id, use_element, selected) {
        return None;
    }
    Some(crate::elements::CanonicalPattern {
        image: crate::elements::Image {
            x: crate::attributes::parse_float(image.attribute("x"), 0.0),
            y: crate::attributes::parse_float(image.attribute("y"), 0.0),
            width: crate::attributes::parse_float(Some(width), 0.0),
            height: crate::attributes::parse_float(Some(height), 0.0),
            href: image.attribute("xlink:href").map(str::to_string),
        },
        id: format!("{id}-ref"),
    })
}

fn pattern_reference_check(
    node: &vectory::Element,
    id: &str,
    internal_use: &vectory::Element,
    selected: bool,
) -> bool {
    if selected {
        return selected_pattern_reference(node, &format!("{id}-ref"));
    }
    !referenced_pattern(node, &[id.to_string(), format!("{id}-ref")], internal_use)
}

fn selected_pattern_reference(node: &vectory::Element, id: &str) -> bool {
    let supported = ["rect", "ellipse", "circle", "polygon", "polyline", "path"];
    if supported.contains(&node.local_name())
        && matches!(node.attribute("fill"), Some(value) if value == format!("#{id}") || value == format!("url(#{id})"))
    {
        return true;
    }
    node.child_elements()
        .any(|child| selected_pattern_reference(child, id))
}

fn referenced_pattern(
    node: &vectory::Element,
    ids: &[String],
    internal_use: &vectory::Element,
) -> bool {
    if std::ptr::eq(node, internal_use) {
        return false;
    }
    if node.local_name() == "style" {
        return true;
    }
    let referenced = node.attributes.iter().any(|attribute| {
        ids.iter().any(|id| {
            ((attribute.name == "href"
                || attribute.name == "xlink:href"
                || attribute.name == "fill")
                && attribute.value == format!("#{id}"))
                || value_references(&attribute.value, id)
        })
    });
    referenced
        || node
            .child_elements()
            .any(|child| referenced_pattern(child, ids, internal_use))
}

/// `url( ["']? #id ["']? )` with optional interior whitespace.
fn value_references(value: &str, id: &str) -> bool {
    let Some(rest) = value.strip_prefix("url(") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(')') else {
        return false;
    };
    let rest = rest.trim();
    let rest = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .or_else(|| rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
        .unwrap_or(rest);
    rest.trim() == format!("#{id}")
}

fn canonical_image_attributes(
    image: &vectory::Element,
    id: &str,
    width: &str,
    height: &str,
) -> bool {
    if image.child_elements().next().is_some() {
        return false;
    }
    let Some(href) = image.attribute("xlink:href") else {
        return false;
    };
    if !href.starts_with("data:image/png;base64,") {
        return false;
    }
    attribute_pairs(image)
        == vec![
            ("id".to_string(), id.to_string()),
            ("x".to_string(), "0".to_string()),
            ("y".to_string(), "0".to_string()),
            ("width".to_string(), width.to_string()),
            ("height".to_string(), height.to_string()),
            ("xlink:href".to_string(), href.to_string()),
            ("preserveAspectRatio".to_string(), "none".to_string()),
        ]
}

fn attribute_pairs(node: &vectory::Element) -> Vec<(String, String)> {
    node.attributes
        .iter()
        .map(|a| (a.name.clone(), a.value.clone()))
        .collect()
}

fn is_image_id(id: &str) -> bool {
    let Some(digits) = id.strip_prefix("img-") else {
        return false;
    };
    !digits.is_empty() && !digits.starts_with('0') && digits.chars().all(|c| c.is_ascii_digit())
}

fn positive_integer(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('0') && value.chars().all(|c| c.is_ascii_digit())
}

/// Command list equality by (letter, args), for the pending-clip match.
#[must_use]
pub fn commands_match(a: &[Command], b: &[Command]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| left.letter == right.letter && left.args == right.args)
}

#[cfg(test)]
mod golden_153 {
    use super::*;

    const TAG: &str = "<path d=\"M 251.0000,-670.3333 L 251.0000,-553.0000 L 413.0000,-553.6667 L 422.3333,-486.0000 L 459.6667,-551.3333 L 490.6667,-551.3333 L 490.6667,-670.3333 L 251.0000,-670.3333 Z \"  fill=\"#FFFFFF\" stroke=\"#FFFFFF\" stroke-width=\"2.6667\"  stroke-linecap=\"round\"  stroke-linejoin=\"round\" />";
    const D: &str = "M 251.0000,-670.3333 L 251.0000,-553.0000 L 413.0000,-553.6667 L 422.3333,-486.0000 L 459.6667,-551.3333 L 490.6667,-551.3333 L 490.6667,-670.3333 L 251.0000,-670.3333 Z ";

    #[test]
    fn poly_polygon_provenance_matches() {
        let mut node = vectory::Element::new("path");
        node.set_attribute("d", D);
        node.set_attribute("fill", "#FFFFFF");
        node.set_attribute("stroke", "#FFFFFF");
        node.set_attribute("stroke-width", "2.6667");
        node.set_attribute("stroke-linecap", "round");
        node.set_attribute("stroke-linejoin", "round");
        assert!(closed_single_contour(D), "closed_single_contour");
        assert!(fill_first_style(&node, Some(TAG)), "fill_first_style");
        assert!(
            canonical_poly_polygon(&node, Some(TAG)),
            "canonical_poly_polygon"
        );
    }

    #[test]
    fn polyline_provenance_matches_the_one_space_form() {
        let tag = "<path  clip-path=\"url(#clip-1144108930)\" d=\"M 67.0051,340.0269 L 68.0295,340.0269 \" stroke=\"#FFFFFF\" stroke-width=\"1px\"  stroke-linecap=\"round\"  stroke-linejoin=\"round\" fill=\"none\" />";
        let d = "M 67.0051,340.0269 L 68.0295,340.0269 ";
        let mut node = vectory::Element::new("path");
        node.set_attribute("clip-path", "url(#clip-1144108930)");
        node.set_attribute("d", d);
        node.set_attribute("stroke", "#FFFFFF");
        node.set_attribute("stroke-width", "1px");
        node.set_attribute("stroke-linecap", "round");
        node.set_attribute("stroke-linejoin", "round");
        node.set_attribute("fill", "none");
        assert!(two_point_line(d), "two_point_line");
        assert!(stroked_path(&node, Some(tag), false, false), "stroked_path");
        assert!(canonical_polyline(&node, Some(tag)), "canonical_polyline");
    }

    #[test]
    fn polyline_provenance_rejects_the_two_space_form() {
        let tag = "<path  clip-path=\"url(#clip-470211272)\" d=\"M 100.9897,332.9769 L 102.0141,332.9769 \" stroke=\"#C0C0C0\" stroke-width=\"1px\"  stroke-linecap=\"round\"  stroke-linejoin=\"round\"  fill=\"none\" />";
        let d = "M 100.9897,332.9769 L 102.0141,332.9769 ";
        let mut node = vectory::Element::new("path");
        node.set_attribute("clip-path", "url(#clip-470211272)");
        node.set_attribute("d", d);
        node.set_attribute("stroke", "#C0C0C0");
        node.set_attribute("stroke-width", "1px");
        node.set_attribute("stroke-linecap", "round");
        node.set_attribute("stroke-linejoin", "round");
        node.set_attribute("fill", "none");
        assert!(
            !canonical_polyline(&node, Some(tag)),
            "canonical_polyline must reject the two-space form"
        );
    }
}
