//! The resolved SVG element tree: one variant per supported element with
//! its typed fields, parsed from the vectory DOM with the reference
//! `Svg::Elements` factories (including the canonical provenance forms the
//! renderer's exact spellings recover).

use std::collections::HashMap;

use crate::paint::{Paint, Stroke};
use eikogram::Matrix;

/// An element's resolved paint pair plus its clip reference.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub fill: Paint,
    pub stroke: Option<Stroke>,
    pub clip_path: Option<String>,
}

/// A pattern definition the defs provenance recovered.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalPattern {
    pub image: Image,
    pub id: String,
}

/// One resolved element. Children are node indices into the tree vector.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: usize,
    pub kind: Kind,
    pub children: Vec<usize>,
}

/// The supported element kinds with their resolved fields.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: f64,
        ry: f64,
        style: Style,
        /// The renderer's rectangle provenance: the textual attribute
        /// values.
        serialization_hint: Option<[String; 5]>,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        style: Style,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        style: Style,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        style: Style,
    },
    Polyline {
        points: Vec<(f64, f64)>,
        style: Style,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        style: Style,
    },
    Path {
        commands: Vec<crate::path_data::Command>,
        style: Style,
        /// The recovered line-record family (the continuation spelling).
        line_record_kind: Option<LineKind>,
        /// The recovered record family of the whole path.
        path_record_kind: Option<PathKind>,
    },
    Group {
        transform: Matrix,
        legacy_broken_y: bool,
        canonical_matrix: bool,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        font_family: Option<String>,
        font_size: f64,
        font_weight: i64,
        italic: bool,
        text_anchor: String,
        style: Style,
        transform: Matrix,
        empty_cdata_content: bool,
        canonical_rotation: Option<crate::ingest_text::Rotation>,
    },
    Image {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        href: Option<String>,
        style: Style,
    },
    Defs {
        unused_pattern: Option<CanonicalPattern>,
        selected_pattern: Option<CanonicalPattern>,
    },
    ClipPath {
        clip_id: Option<String>,
        unused_rectangle_bounds: Option<[f64; 4]>,
        /// The general clip region bounds (the rectangular form when the
        /// clip traces one; the compound fallback otherwise).
        clip_bounds: Option<[f64; 4]>,
        selected_path_data: Option<String>,
    },
    /// An unsupported element; traversal still descends.
    Open { name: String },
}

/// The line-record family a path's spelling recovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// A standalone polyline (its own current-position move).
    Polyline,
    /// A continuation from the carried current position.
    PolylineTo,
}

/// The record family a path's spelling recovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    EndPath,
    FillPath,
    BitBlt,
    PolyPolygon,
    StrokeAndFillPath,
    PolyBezierTo,
    StrokePath,
    Arc,
    Chord,
    Pie,
}

impl Kind {
    /// The element's style, when it carries one.
    #[must_use]
    pub fn style(&self) -> Option<&Style> {
        match self {
            Kind::Rect { style, .. }
            | Kind::Ellipse { style, .. }
            | Kind::Circle { style, .. }
            | Kind::Line { style, .. }
            | Kind::Polyline { style, .. }
            | Kind::Polygon { style, .. }
            | Kind::Path { style, .. }
            | Kind::Text { style, .. }
            | Kind::Image { style, .. } => Some(style),
            _ => None,
        }
    }

    /// The element's clip-path reference, when any.
    #[must_use]
    pub fn clip_path(&self) -> Option<&str> {
        self.style().and_then(|style| style.clip_path.as_deref())
    }
}

/// An `<image>` element's resolved payload fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub href: Option<String>,
}

impl Image {
    /// Whether the href is a data URI.
    #[must_use]
    pub fn data_uri(&self) -> bool {
        self.href
            .as_deref()
            .is_some_and(|href| href.starts_with("data:"))
    }

    /// The URI's media type.
    #[must_use]
    pub fn mime_type(&self) -> Option<String> {
        let href = self.href.as_deref()?;
        let rest = href.strip_prefix("data:")?;
        let end = rest.find([';', ','])?;
        Some(rest[..end].to_string())
    }

    /// The decoded data-URI bytes (base64 or plain).
    #[must_use]
    pub fn decoded_bytes(&self) -> Option<Vec<u8>> {
        let href = self.href.as_deref()?;
        let rest = href.strip_prefix("data:")?;
        let comma = rest.find(',')?;
        let payload = &rest[comma + 1..];
        if href.contains(";base64,") {
            base64_decode(payload)
        } else {
            Some(payload.as_bytes().to_vec())
        }
    }
}

/// The tree: resolved elements with parent-of relationships implicit in
/// the child id lists.
pub struct Tree {
    pub elements: Vec<Element>,
    pub tags: HashMap<usize, Option<String>>,
    pub root: usize,
}

impl Tree {
    /// The kind of a node.
    #[must_use]
    pub fn kind(&self, id: usize) -> &Kind {
        &self.elements[id].kind
    }

    /// A short kind name for parity debugging (not an API contract).
    #[must_use]
    pub fn kind_name(&self, id: usize) -> &'static str {
        match &self.elements[id].kind {
            Kind::Rect { .. } => "rect",
            Kind::Ellipse { .. } => "ellipse",
            Kind::Circle { .. } => "circle",
            Kind::Line { .. } => "line",
            Kind::Polyline { .. } => "polyline",
            Kind::Polygon { .. } => "polygon",
            Kind::Path { .. } => "path",
            Kind::Group { .. } => "g",
            Kind::Text { .. } => "text",
            Kind::Image { .. } => "image",
            Kind::Defs { .. } => "defs",
            Kind::ClipPath { .. } => "clipPath",
            Kind::Open { .. } => "open",
        }
    }

    /// The clip-path attribute of a node, for parity debugging.
    #[must_use]
    pub fn clip_attr(&self, id: usize) -> Option<&str> {
        self.elements[id].kind.clip_path()
    }

    /// The child ids of a node.
    #[must_use]
    pub fn children(&self, id: usize) -> &[usize] {
        &self.elements[id].children
    }

    /// The raw source tag a resolved element carried (path/rect only).
    #[must_use]
    pub fn tag(&self, id: usize) -> Option<&str> {
        self.tags.get(&id)?.as_deref()
    }
}

/// The legacy broken-Y literal the reference writes for the repaired
/// header (invalid SVG translate syntax, carried verbatim).
pub const LEGACY_BROKEN_Y_TRANSFORM: &str = "translate(0.0000, 0.00 00)";

/// Parses the vectory DOM into the resolved tree, attaching the raw
/// source tags of `path`/`rect` elements (the provenance forms read them).
///
/// # Errors
///
/// Fails when a colour, path datum, or transform is malformed, as the
/// reference raises.
pub fn build_tree(document: &vectory::Document, source: &str) -> Result<Tree, String> {
    let source_tags = collect_source_tags(source);
    let mut tag_index = 0usize;
    let mut builder = TreeBuilder {
        elements: Vec::new(),
        tags: HashMap::new(),
        source: source_tags,
        tag_index: &mut tag_index,
    };
    let root = match &document.root {
        Some(root) => builder.build(root, root)?,
        None => return Err("missing root element".to_string()),
    };
    Ok(Tree {
        elements: builder.elements,
        tags: builder.tags,
        root,
    })
}

struct TreeBuilder<'a> {
    elements: Vec<Element>,
    tags: HashMap<usize, Option<String>>,
    source: Vec<String>,
    tag_index: &'a mut usize,
}

impl TreeBuilder<'_> {
    fn build(&mut self, node: &vectory::Element, root: &vectory::Element) -> Result<usize, String> {
        let name = node.local_name().to_string();
        if name == "clipPath" {
            return self.clip_path(node, root);
        }
        let kind = match name.as_str() {
            "rect" => self.rect(node)?,
            "ellipse" => self.ellipse(node)?,
            "circle" => self.circle(node)?,
            "line" => self.line(node)?,
            "polyline" => self.polyline(node)?,
            "polygon" => self.polygon(node)?,
            "path" => self.path(node)?,
            "g" => self.group(node)?,
            "text" => self.text(node)?,
            "image" => self.image(node)?,
            "defs" => self.defs(node, root)?,
            other => Kind::Open {
                name: other.to_string(),
            },
        };
        let mut children = Vec::new();
        for child in node.child_elements() {
            children.push(self.build(child, root)?);
        }
        let id = self.elements.len();
        self.elements.push(Element { id, kind, children });
        Ok(id)
    }

    fn style(&self, node: &vectory::Element) -> Result<Style, String> {
        let fill = Paint::from_fill(node.attribute("fill"))?;
        let stroke = Stroke::parse(
            node.attribute("stroke"),
            node.attribute("stroke-width"),
            node.attribute("stroke-dasharray"),
            node.attribute("stroke-linecap"),
            node.attribute("stroke-linejoin"),
            node.attribute("stroke-miterlimit"),
            &fill,
        )?;
        Ok(Style {
            fill,
            stroke: Some(stroke),
            clip_path: node.attribute("clip-path").map(str::to_string),
        })
    }

    fn rect(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        let style = self.style(node)?;
        let source_tag = self.next_source_tag();
        let serialization_hint = crate::hints::rectangle_values(node);
        let id = self.elements.len();
        self.tags.insert(id, source_tag);
        Ok(Kind::Rect {
            x: crate::attributes::parse_float(node.attribute("x"), 0.0),
            y: crate::attributes::parse_float(node.attribute("y"), 0.0),
            width: crate::attributes::parse_float(node.attribute("width"), 0.0),
            height: crate::attributes::parse_float(node.attribute("height"), 0.0),
            rx: crate::attributes::parse_float(node.attribute("rx"), 0.0),
            ry: crate::attributes::parse_float(node.attribute("ry"), 0.0),
            style,
            serialization_hint,
        })
    }

    fn ellipse(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Ellipse {
            cx: crate::attributes::parse_float(node.attribute("cx"), 0.0),
            cy: crate::attributes::parse_float(node.attribute("cy"), 0.0),
            rx: crate::attributes::parse_float(node.attribute("rx"), 0.0),
            ry: crate::attributes::parse_float(node.attribute("ry"), 0.0),
            style: self.style(node)?,
        })
    }

    fn circle(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Circle {
            cx: crate::attributes::parse_float(node.attribute("cx"), 0.0),
            cy: crate::attributes::parse_float(node.attribute("cy"), 0.0),
            r: crate::attributes::parse_float(node.attribute("r"), 0.0),
            style: self.style(node)?,
        })
    }

    fn line(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Line {
            x1: crate::attributes::parse_float(node.attribute("x1"), 0.0),
            y1: crate::attributes::parse_float(node.attribute("y1"), 0.0),
            x2: crate::attributes::parse_float(node.attribute("x2"), 0.0),
            y2: crate::attributes::parse_float(node.attribute("y2"), 0.0),
            style: self.style(node)?,
        })
    }

    fn polyline(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Polyline {
            points: crate::attributes::parse_points(node.attribute("points")),
            style: self.style(node)?,
        })
    }

    fn polygon(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Polygon {
            points: crate::attributes::parse_points(node.attribute("points")),
            style: self.style(node)?,
        })
    }

    fn path(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        let source_tag = self.next_source_tag();
        let id = self.elements.len();
        self.tags.insert(id, source_tag.clone());
        let mut style = self.style(node)?;
        if let Some(stroke) = style.stroke.as_mut() {
            stroke.with_canonical_cosmetic_dash(
                node.attribute("d"),
                node.attribute("stroke"),
                node.attribute("stroke-dasharray"),
                node.attribute("stroke-linecap"),
                node.attribute("stroke-linejoin"),
                node.attribute("fill"),
            );
        }
        let tag = source_tag.as_deref();
        let path_record_kind = crate::provenance::path_record_kind(node, tag);
        let brush_fallback = crate::provenance::brush_fallback(node, tag);
        if brush_fallback {
            // The fallback stroke reconstructs from defaults plus the
            // carried paint (the reference constructs, not re-parses).
            let paint = style
                .stroke
                .as_ref()
                .map(|stroke| stroke.paint.clone())
                .unwrap_or(Paint::Null);
            style.stroke = Some(crate::ingest::stroke_for_fallback(paint));
        }
        if path_record_kind == Some(PathKind::BitBlt) {
            let style_value = node.attribute("style").unwrap_or("");
            style.fill = Paint::from_fill(style_value.strip_prefix("fill:"))?;
        }
        let line_record_kind = if brush_fallback {
            Some(LineKind::Polyline)
        } else {
            crate::provenance::line_record_kind(node, tag)
        };
        Ok(Kind::Path {
            commands: crate::path_data::parse(node.attribute("d"))?,
            style,
            line_record_kind,
            path_record_kind,
        })
    }

    fn group(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        let transform = crate::transform::parse(node.attribute("transform"))?;
        Ok(Kind::Group {
            transform,
            legacy_broken_y: node.attribute("transform") == Some(LEGACY_BROKEN_Y_TRANSFORM),
            canonical_matrix: crate::provenance::canonical_matrix_node(node),
        })
    }

    fn text(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        let rotation = crate::ingest_text::canonical_rotation(node);
        let content = node_text(node);
        let empty_cdata = node
            .children
            .iter()
            .any(|child| matches!(child, vectory::Node::CData(content) if content.is_empty()));
        Ok(Kind::Text {
            x: crate::attributes::parse_float(node.attribute("x"), 0.0),
            y: crate::attributes::parse_float(node.attribute("y"), 0.0),
            content,
            font_family: node.attribute("font-family").map(str::to_string),
            font_size: crate::attributes::parse_float(node.attribute("font-size"), 12.0),
            font_weight: parse_weight(node.attribute("font-weight")),
            italic: node.attribute("font-style") == Some("italic"),
            text_anchor: node.attribute("text-anchor").unwrap_or("start").to_string(),
            style: self.style(node)?,
            transform: if rotation.is_some() {
                Matrix::identity()
            } else {
                crate::transform::parse(node.attribute("transform"))?
            },
            empty_cdata_content: empty_cdata,
            canonical_rotation: rotation,
        })
    }

    fn image(&mut self, node: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Image {
            x: crate::attributes::parse_float(node.attribute("x"), 0.0),
            y: crate::attributes::parse_float(node.attribute("y"), 0.0),
            width: crate::attributes::parse_float(node.attribute("width"), 0.0),
            height: crate::attributes::parse_float(node.attribute("height"), 0.0),
            href: node
                .attribute("xlink:href")
                .or_else(|| node.attribute("href"))
                .map(str::to_string),
            style: self.style(node)?,
        })
    }

    fn defs(&mut self, node: &vectory::Element, root: &vectory::Element) -> Result<Kind, String> {
        Ok(Kind::Defs {
            unused_pattern: crate::provenance::canonical_pattern(node, root, false),
            selected_pattern: crate::provenance::canonical_pattern(node, root, true),
        })
    }

    fn clip_path(&mut self, node: &vectory::Element, root: &vectory::Element) -> Result<usize, String> {
        // The children build first (their source tags feed the canonical
        // checks), then the clip kind.
        let mut children = Vec::new();
        for child in node.child_elements() {
            children.push(self.build(child, root)?);
        }
        let child_node = node.child_elements().next();
        let unused_rectangle_bounds =
            crate::provenance::canonical_unused_rectangle(node, child_node);
        let selected_path_data = crate::provenance::canonical_selected_path(node, child_node);
        let clip_bounds = clip_bounds_of(self, &children);
        let id = self.elements.len();
        self.elements.push(Element {
            id,
            kind: Kind::ClipPath {
                clip_id: node.attribute("id").map(str::to_string),
                unused_rectangle_bounds,
                clip_bounds,
                selected_path_data,
            },
            children,
        });
        Ok(id)
    }

    fn next_source_tag(&mut self) -> Option<String> {
        let tag = self.source.get(*self.tag_index).cloned();
        if tag.is_some() {
            *self.tag_index += 1;
        }
        tag
    }
}

/// The clip region bounds, computed once per definition (the rectangular
/// form when the clip traces one; the compound fallback otherwise).
fn clip_bounds_of(builder: &TreeBuilder<'_>, children: &[usize]) -> Option<[f64; 4]> {
    if children.is_empty() {
        return None;
    }
    if children.len() == 1 {
        let child = &builder.elements[children[0]].kind;
        if let Kind::Rect {
            x,
            y,
            width,
            height,
            ..
        } = child
        {
            return Some([*x, *y, x + width, y + height]);
        }
        if let Kind::Path { commands, .. } = child {
            return path_rect_bounds(commands);
        }
    }
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for child in children {
        match &builder.elements[*child].kind {
            Kind::Rect {
                x,
                y,
                width,
                height,
                ..
            } => {
                xs.push(*x);
                xs.push(x + width);
                ys.push(*y);
                ys.push(y + height);
            }
            Kind::Ellipse { cx, cy, rx, ry, .. } => {
                xs.push(cx - rx);
                xs.push(cx + rx);
                ys.push(cy - ry);
                ys.push(cy + ry);
            }
            Kind::Circle { cx, cy, r, .. } => {
                xs.push(cx - r);
                xs.push(cx + r);
                ys.push(cy - r);
                ys.push(cy + r);
            }
            Kind::Path { commands, .. } => {
                let Some(points) = crate::flattener::PathFlatten::flatten_for_clip(commands) else {
                    continue;
                };
                for (x, y) in points {
                    xs.push(x);
                    ys.push(y);
                }
            }
            _ => {}
        }
    }
    if xs.is_empty() {
        return None;
    }
    Some([
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        ys.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ])
}

/// The single-contour rectangular detection: the renderer's IntersectClipRect
/// form traces a four-corner path.
fn path_rect_bounds(commands: &[crate::path_data::Command]) -> Option<[f64; 4]> {
    if commands.iter().filter(|c| c.upcased() == "M").count() != 1 {
        return None;
    }
    let points = crate::flattener::PathFlatten::flatten_for_clip(commands)?;
    if points.len() < 4 {
        return None;
    }
    let xs: Vec<f64> = points.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = points.iter().map(|p| p.1).collect();
    let bounds = [
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        ys.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ];
    // Preserve the single-contour fallback outside the four-corner case.
    let mut unique = points.clone();
    unique.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    unique.dedup();
    if unique.len() != 4 {
        return Some(bounds);
    }
    let letters: String = commands.iter().map(|c| c.upcased()).collect();
    let mut rest = letters.as_str();
    if let Some(stripped) = rest.strip_prefix('M') {
        rest = stripped;
    } else {
        return Some(bounds);
    }
    let line_or_hv = rest.chars().all(|c| matches!(c, 'L' | 'H' | 'V' | 'Z'));
    if !line_or_hv || !letters.contains('Z') {
        return Some(bounds);
    }
    let mut contour = points.clone();
    if contour.last() == contour.first() {
        contour.pop();
    }
    if contour.len() != 4 {
        return None;
    }
    let orthogonal = (0..4).all(|index| {
        let a = contour[index];
        let b = contour[(index + 1) % 4];
        (a.0 == b.0) != (a.1 == b.1)
    });
    let two_x = {
        let mut values = xs.clone();
        values.sort_by(|a, b| a.total_cmp(b));
        values.dedup();
        values.len() == 2
    };
    let two_y = {
        let mut values = ys.clone();
        values.sort_by(|a, b| a.total_cmp(b));
        values.dedup();
        values.len() == 2
    };
    if !(orthogonal && two_x && two_y) {
        return None;
    }
    Some(bounds)
}

/// The raw start tags of every `path`/`rect` element in document order
/// (the reference anchors the same scan at each node's byte offset).
fn collect_source_tags(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let rest = &source[index..];
        let matches_tag = rest.len() >= 5
            && ((rest.starts_with("<path") && boundary(rest, 5))
                || (rest.starts_with("<rect") && boundary(rest, 5)));
        if !matches_tag {
            index += 1;
            continue;
        }
        let mut cursor = index + 1;
        let mut closed = false;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'"' => {
                    cursor += 1;
                    while cursor < bytes.len() && bytes[cursor] != b'"' {
                        cursor += 1;
                    }
                }
                b'\'' => {
                    cursor += 1;
                    while cursor < bytes.len() && bytes[cursor] != b'\'' {
                        cursor += 1;
                    }
                }
                b'>' => {
                    closed = true;
                    break;
                }
                _ => {}
            }
            cursor += 1;
        }
        if closed {
            out.push(source[index..=cursor].to_string());
            index = cursor + 1;
        } else {
            index += 1;
        }
    }
    out
}

fn boundary(rest: &str, at: usize) -> bool {
    rest[at..]
        .chars()
        .next()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'))
}

/// The concatenated text content of an element (text nodes and CDATA).
fn node_text(node: &vectory::Element) -> String {
    let mut out = String::new();
    for child in &node.children {
        match child {
            vectory::Node::Text(text) => out.push_str(&text.0),
            vectory::Node::CData(content) => out.push_str(content),
            _ => {}
        }
    }
    out
}

fn parse_weight(value: Option<&str>) -> i64 {
    match value {
        None => 0,
        Some("normal") => 400,
        Some("bold") => 700,
        Some(other) => other.trim().parse::<i64>().unwrap_or(0),
    }
}

/// Base64 decoding with the reference's tolerant trailing handling.
#[must_use]
pub fn base64_decode(payload: &str) -> Option<Vec<u8>> {
    let mut table = [255u8; 256];
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (index, &byte) in ALPHABET.iter().enumerate() {
        table[byte as usize] = index as u8;
    }
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in payload.bytes() {
        if byte == b'=' || byte == b'\n' || byte == b'\r' {
            continue;
        }
        let value = table[byte as usize];
        if value == 255 {
            return None;
        }
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}
