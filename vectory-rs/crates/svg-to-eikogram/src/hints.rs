//! Drawing-hint resolution: the provenance payloads a drawn run carries
//! (the rect textual values, the cosmetic-stroke colour, the jpeg flag),
//! mirroring the reference `DrawingHint.for_element`.

/// The reference rectangle spelling's textual values: the hint carries the
/// exact attribute forms (they are not derivable from the parsed floats).
#[must_use]
pub fn rectangle_values(node: &vectory::Element) -> Option<[String; 5]> {
    // The renderer's bare-rectangle spelling: the exact attribute
    // sequence x, y, width, height, fill (the whitespace-insensitive
    // model form of the reference's source-tag template).
    let names: Vec<&str> = node.attributes.iter().map(|a| a.name.as_str()).collect();
    if names != ["x", "y", "width", "height", "fill"] {
        return None;
    }
    let x = node.attribute("x")?;
    let y = node.attribute("y")?;
    let width = node.attribute("width")?;
    let height = node.attribute("height")?;
    let fill = node.attribute("fill")?;
    for value in [x, y, width, height] {
        if !is_loose_number(value) {
            return None;
        }
    }
    let hex = fill.strip_prefix('#')?;
    if hex.len() != 6
        || !hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
    {
        return None;
    }
    Some([
        x.to_string(),
        y.to_string(),
        width.to_string(),
        height.to_string(),
        format!("#{hex}"),
    ])
}

fn is_loose_number(value: &str) -> bool {
    let body = value.strip_prefix('-').unwrap_or(value);
    let Some((integers, decimals)) = body.split_once('.') else {
        return !body.is_empty() && body.chars().all(|c| c.is_ascii_digit());
    };
    !integers.is_empty()
        && integers.chars().all(|c| c.is_ascii_digit())
        && !decimals.is_empty()
        && decimals.chars().all(|c| c.is_ascii_digit())
}
