//! # svg-to-eikogram — the SVG-model → VIM transformer
//!
//! Ingests a `vectory::Document` (the SVG model, consumed as the RUST
//! model) into a `eikogram::Drawing`, the format-neutral Vector Image
//! Model (claricle/emf#90; the reverse lane of the conversion hub, the
//! mirror of eikogram-to-svg).
//!
//! The semantics ported here are the Ruby reference translation's exact
//! choices (emfsvg's Translation::EmfRenderer + handlers + the Svg::
//! resolution layer), one to one:
//!
//! * attribute resolution — colours, strokes with their provenance forms
//!   (the null-pen sentinel, the canonical cosmetic dash, the
//!   brush-fallback spelling), transforms, path data;
//! * provenance recovery — the renderer's exact spellings that carry
//!   record families, clip plans, and hint payloads back through the
//!   round trip;
//! * the mapping choice — the fixed device scale (with the overflow
//!   fallback to the identity mapping) and its prelude;
//! * the clip plans — the sibling carry, selected, pending, and interval
//!   scopes over the resolved tree;
//! * the run structure — the shared GDI lifecycle with its scope and
//!   text-state reuse, resolved against the item list;
//! * the drawing programs — poly families, figures, native circular
//!   arcs (with the mapped-origin recoveries), direct blits;
//! * the canonical text facts — the recovered escapement, alignment, and
//!   native grid values, with the finer device mapping.
//!
//! # Boundaries
//!
//! The transformer is the only place the SVG model and the VIM meet; it
//! reads the vectory model only and never touches EMF. The lowering lives
//! in `eikogram-to-emf` (the parity gate, in tests, composes both).
//!
//! # Errors
//!
//! [`svg_to_eikogram`] fails on the reference translation's raised
//! conditions: malformed colours, path data, or transforms, and the
//! scaled-range overflow (which the caller-side fallback resolves by
//! re-running under the identity mapping, as the reference does).

mod arcs;
mod attributes;
mod clip_registry;
mod color;
mod elements;
mod flattener;
mod hints;
mod ingest;
mod number;
mod paint;
mod path_data;
mod provenance;
mod text;
mod transform;

// The canonical text rotation recovery, aliased for the element builder.
mod ingest_text {
    // Re-exported from [`crate::text`].
    pub use crate::text::{canonical_rotation, Rotation};
}

pub use clip_registry::ClipPathRegistry;
pub use debug::{debug_tree, DebugKindRef};
pub use elements::{build_tree, Tree};
pub use provenance::line_record_kind;
pub use ingest::{ingest, IngestFailure, Scaler, Viewport};

use eikogram::Drawing;

/// The fixed mapping factor (matching the reference's `%.4f` choice).
pub const FIXED_FACTOR: u64 = 10_000;

/// Ingests SVG bytes into a VIM drawing: parses the model, resolves the
/// tree, picks the mapping (falling back to the identity mapping on
/// scaled-range overflow, exactly as the reference retries), and walks.
///
/// # Errors
///
/// Returns the reference translation's raised conditions (parse failures,
/// malformed values); the fixed-scale overflow is resolved internally by
/// the identity-mapping retry.
pub fn svg_to_eikogram(content: &str) -> Result<Drawing, IngestFailure> {
    let document = vectory::Document::parse(content)
        .map_err(|error| IngestFailure::Format(error.to_string()))?;
    document_to_eikogram(&document, content)
}

/// Ingests a parsed SVG model document. The raw source rides along for
/// the provenance forms (they read the exact start-tag spellings).
///
/// # Errors
///
/// See [`svg_to_eikogram`].
pub fn document_to_eikogram(
    document: &vectory::Document,
    source: &str,
) -> Result<Drawing, IngestFailure> {
    let Some(root) = &document.root else {
        return Err(IngestFailure::Format("missing root element".to_string()));
    };
    let viewport = Viewport {
        width: crate::attributes::parse_float(root.attribute("width"), 0.0),
        height: crate::attributes::parse_float(root.attribute("height"), 0.0),
        view_box: parse_view_box(root.attribute("viewBox")),
    };
    let tree = elements::build_tree(document, source).map_err(IngestFailure::Format)?;
    // The fixed scale keeps decimal coordinates intact; when any value
    // would leave the scaled range, the identity mapping runs instead.
    if needs_scaling(&tree) && safe_for_fixed_scale(&tree).is_ok() {
        if let Ok(drawing) = ingest(&tree, viewport, Scaler::Fixed(FIXED_FACTOR)) {
            return Ok(drawing);
        }
    }
    ingest(&tree, viewport, Scaler::Identity)
}

fn parse_view_box(value: Option<&str>) -> Option<[f64; 4]> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let parts: Vec<f64> = value
        .split([' ', '\t', '\n', '\r', ','])
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect();
    (parts.len() == 4).then_some([parts[0], parts[1], parts[2], parts[3]])
}

/// Whether any coordinate carries decimals (or the explicit cosmetic
/// one-pixel stroke appears), so the fixed mapping preserves them.
fn needs_scaling(tree: &elements::Tree) -> bool {
    any_decimal(tree, tree.root)
}

fn any_decimal(tree: &elements::Tree, id: usize) -> bool {
    let kind = tree.kind(id);
    if explicit_cosmetic_pixel_stroke(kind) {
        return true;
    }
    let decimal = match kind {
        elements::Kind::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
            ..
        } => [x, y, width, height, rx, ry]
            .iter()
            .any(|value| is_decimal(**value)),
        elements::Kind::Ellipse { cx, cy, rx, ry, .. } => {
            [*cx, *cy, *rx, *ry].iter().any(|value| is_decimal(*value))
        }
        elements::Kind::Circle { cx, cy, r, .. } => {
            [*cx, *cy, *r].iter().any(|value| is_decimal(*value))
        }
        elements::Kind::Line { x1, y1, x2, y2, .. } => {
            [x1, y1, x2, y2].iter().any(|value| is_decimal(**value))
        }
        elements::Kind::Polyline { points, .. } | elements::Kind::Polygon { points, .. } => {
            points.iter().any(|(x, y)| is_decimal(*x) || is_decimal(*y))
        }
        elements::Kind::Path { commands, .. } => commands
            .iter()
            .any(|command| command.args.iter().any(|value| is_decimal(*value))),
        elements::Kind::Text {
            x, y, font_size, ..
        } => [x, y, font_size].iter().any(|value| is_decimal(**value)),
        elements::Kind::Image {
            x,
            y,
            width,
            height,
            ..
        } => [x, y, width, height]
            .iter()
            .any(|value| is_decimal(**value)),
        _ => false,
    };
    if decimal {
        return true;
    }
    tree.children(id)
        .iter()
        .any(|child| any_decimal(tree, *child))
}

/// The `value != value.to_i` decimal test on the parsed floats.
fn is_decimal(value: f64) -> bool {
    !value.is_finite() || value != value.trunc()
}

/// The explicit cosmetic one-pixel stroke (a real pen, not the fallback).
fn explicit_cosmetic_pixel_stroke(kind: &elements::Kind) -> bool {
    let Some(style) = kind.style() else {
        return false;
    };
    let Some(stroke) = &style.stroke else {
        return false;
    };
    stroke.raw_width == "1px"
        && (stroke.line_cap_explicit || stroke.line_join_explicit)
        && !stroke.paint.null()
}

/// Whether every scaled value stays inside int32 (the fixed-scale safety).
fn safe_for_fixed_scale(tree: &elements::Tree) -> Result<(), IngestFailure> {
    any_unsafe(tree, tree.root, Scaler::Fixed(FIXED_FACTOR))
}

fn any_unsafe(tree: &elements::Tree, id: usize, scaler: Scaler) -> Result<(), IngestFailure> {
    let kind = tree.kind(id);
    // Paths with a local origin remap their values safely.
    if local_origin_exempt(tree, id, scaler) {
        return Ok(());
    }
    let unsafe_here = match kind {
        elements::Kind::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
            ..
        } => [x, y, width, height, rx, ry]
            .iter()
            .any(|value| scaler.unsafe_value(**value)),
        elements::Kind::Ellipse { cx, cy, rx, ry, .. } => [*cx, *cy, *rx, *ry]
            .iter()
            .any(|value| scaler.unsafe_value(*value)),
        elements::Kind::Circle { cx, cy, r, .. } => [*cx, *cy, *r]
            .iter()
            .any(|value| scaler.unsafe_value(*value)),
        elements::Kind::Line { x1, y1, x2, y2, .. } => [x1, y1, x2, y2]
            .iter()
            .any(|value| scaler.unsafe_value(**value)),
        elements::Kind::Polyline { points, .. } | elements::Kind::Polygon { points, .. } => points
            .iter()
            .any(|(x, y)| scaler.unsafe_value(*x) || scaler.unsafe_value(*y)),
        elements::Kind::Path { commands, .. } => commands
            .iter()
            .any(|command| command.args.iter().any(|value| scaler.unsafe_value(*value))),
        elements::Kind::Text {
            x, y, font_size, ..
        } => [x, y, font_size]
            .iter()
            .any(|value| scaler.unsafe_value(**value)),
        elements::Kind::Image {
            x,
            y,
            width,
            height,
            ..
        } => [x, y, width, height]
            .iter()
            .any(|value| scaler.unsafe_value(**value)),
        _ => false,
    };
    if unsafe_here {
        return Err(IngestFailure::Overflow);
    }
    for child in tree.children(id) {
        any_unsafe(tree, *child, scaler)?;
    }
    Ok(())
}

/// The independent-line exemption: a complete absolute line may use a
/// temporary device origin, so its overflow does not disqualify the
/// fixed scale.
fn local_origin_exempt(tree: &elements::Tree, id: usize, scaler: Scaler) -> bool {
    if !scaler.scaled() {
        return false;
    }
    let elements::Kind::Path {
        commands,
        style,
        line_record_kind,
        path_record_kind,
    } = tree.kind(id)
    else {
        return false;
    };
    // independent_line?: no clip, null fill, no record provenance, and
    // exactly an M then an L.
    if !style.clip_path.is_none()
        || !matches!(style.fill, crate::paint::Paint::Null)
        || path_record_kind.is_some()
        || !matches!(line_record_kind, None | Some(elements::LineKind::Polyline))
    {
        return false;
    }
    let letters: Vec<String> = commands.iter().map(|c| c.upcased()).collect();
    if letters != ["M", "L"] || !tree.children(id).is_empty() {
        return false;
    }
    // The origin must make every point safe.
    let points: Vec<(f64, f64)> = commands
        .iter()
        .filter_map(|command| {
            if command.args.len() >= 2 {
                Some((command.args[0], command.args[1]))
            } else {
                None
            }
        })
        .collect();
    if points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return false;
    }
    let mut xs: Vec<f64> = points.iter().map(|(x, _)| *x).collect();
    let mut ys: Vec<f64> = points.iter().map(|(_, y)| *y).collect();
    xs.sort_by(|a, b| a.total_cmp(b));
    ys.sort_by(|a, b| a.total_cmp(b));
    let mut origin = [0_i64; 2];
    for (axis, values) in [(0, &xs), (1, &ys)] {
        let min = values[0];
        let max = values[values.len() - 1];
        if values.iter().any(|value| scaler.unsafe_value(*value)) {
            let offset = ((min + max) / 2.0).floor();
            if !(-2_147_483_648.0..=2_147_483_647.0).contains(&offset) {
                return false;
            }
            origin[axis] = offset as i64;
        }
    }
    points.iter().all(|(x, y)| {
        !scaler.unsafe_value(x - origin[0] as f64) && !scaler.unsafe_value(y - origin[1] as f64)
    })
}

/// Debug introspection for the parity iteration (not a public API
/// contract): the resolved kinds and tags, exposed for tests.
pub mod debug {
    pub use crate::elements::{
        Kind as DebugKind, LineKind as DebugLineKind, PathKind as DebugPathKind,
    };

    /// A borrowed view of a resolved kind, for debug printing.
    pub enum DebugKindRef<'a> {
        Path {
            line: Option<crate::elements::LineKind>,
            path_kind: Option<crate::elements::PathKind>,
            tag: Option<&'a str>,
        },
        Other(&'static str),
    }

    /// The resolved tree, for tests.
    pub struct DebugTree {
        pub elements: Vec<(DebugKind, Option<String>)>,
        pub root: usize,
    }

    impl DebugTree {
        pub fn debug_kind(&self, id: usize) -> DebugKindRef<'_> {
            let (kind, tag) = &self.elements[id];
            match kind {
                DebugKind::Path {
                    line_record_kind,
                    path_record_kind,
                    ..
                } => DebugKindRef::Path {
                    line: *line_record_kind,
                    path_kind: *path_record_kind,
                    tag: tag.as_deref(),
                },
                DebugKind::Rect { .. } => DebugKindRef::Other("rect"),
                DebugKind::Group { .. } => DebugKindRef::Other("g"),
                DebugKind::Text { .. } => DebugKindRef::Other("text"),
                _ => DebugKindRef::Other("other"),
            }
        }

        pub fn kind_summary(&self) -> Vec<(usize, String)> {
            self.elements
                .iter()
                .enumerate()
                .map(|(id, (kind, _))| (id, format!("{kind:?}")))
                .collect()
        }
    }

    /// The defs canonical-pattern recovery result, for parity debugging.
    #[must_use]
    pub fn debug_canonical_pattern(
        node: &vectory::Element,
        root: &vectory::Element,
        selected: bool,
    ) -> bool {
        crate::provenance::canonical_pattern(node, root, selected).is_some()
    }

    /// Builds the debug tree for a source document.
    pub fn debug_tree(document: &vectory::Document, source: &str) -> DebugTree {
        let tree = crate::elements::build_tree(document, source).expect("tree");
        let elements = tree
            .elements
            .iter()
            .map(|element| {
                let tag = tree.tags.get(&element.id).cloned().flatten();
                (element.kind.clone(), tag)
            })
            .collect();
        DebugTree {
            elements,
            root: tree.root,
        }
    }
}
