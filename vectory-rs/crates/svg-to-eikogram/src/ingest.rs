//! The ingest walk: the resolved SVG tree onto VIM items, carrying the
//! reference translation's exact choices — the shared GDI lifecycle per
//! run (with its clip-scope and text-state reuse resolved against the
//! item list), the clip-interval and definition preservation structure,
//! the path drawing programs, the canonical text facts, and the device
//! resolutions (scaler, local origins, pattern payloads).

use std::collections::HashMap;

use crate::clip_registry::{header_group, unused_rectangle_bounds, ClipPathRegistry};
use crate::elements::{Kind, LineKind, PathKind, Style, Tree};
use crate::flattener::{self, SubPath};
use crate::paint::Paint;
use eikogram::{
    ArcForm, ArcKind, Arrangement, Canvas, Color, CoordJoin, DeviceBrush, DevicePaint, DevicePen,
    DevicePoint, DeviceRect, Draw, DrawBody, Drawing, FigurePaint, ImageArrangement, ImageDraw,
    Item, Matrix, PathDraw, PathGeometry, PathProgram, Preamble, Provenance, Raster, RasterFormat,
    RasterPatternDef, Segment, ShapeDraw, ShapeKind, TextAnchor, TextDraw, TextNative, TextRun,
};

/// An ingest failure: the reference translation's raised conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IngestFailure {
    /// A colour, path datum, or transform the reference rejects.
    Format(String),
    /// A coordinate exceeded the scaled int32 range; the caller retries
    /// with the identity mapping.
    Overflow,
}

impl std::fmt::Display for IngestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestFailure::Format(message) => write!(f, "format: {message}"),
            IngestFailure::Overflow => write!(f, "coordinate exceeds the scaled range"),
        }
    }
}

/// The coordinate mapping: direct (identity) or fixed-factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scaler {
    Identity,
    Fixed(u64),
}

impl Scaler {
    #[must_use]
    pub fn factor(self) -> u64 {
        match self {
            Scaler::Identity => 1,
            Scaler::Fixed(factor) => factor,
        }
    }

    #[must_use]
    pub fn scaled(self) -> bool {
        matches!(self, Scaler::Fixed(_))
    }

    /// Maps a canvas value to a device integer.
    ///
    /// # Errors
    ///
    /// [`IngestFailure::Overflow`] when the scaled value leaves int32.
    pub fn to_int(self, value: f64) -> Result<i64, IngestFailure> {
        match self {
            Scaler::Identity => Ok(value.trunc() as i64),
            Scaler::Fixed(factor) => {
                let scaled = value * factor as f64;
                if !scaled.is_finite() {
                    return Err(IngestFailure::Overflow);
                }
                let rounded = scaled.round();
                if !i32::MIN as f64 <= rounded || rounded > i32::MAX as f64 {
                    return Err(IngestFailure::Overflow);
                }
                Ok(rounded as i64)
            }
        }
    }

    /// Whether a value would leave int32 when scaled.
    #[must_use]
    pub fn unsafe_value(self, value: f64) -> bool {
        match self {
            Scaler::Identity => false,
            Scaler::Fixed(factor) => {
                let scaled = value * factor as f64;
                !scaled.is_finite()
                    || !(i32::MIN as f64..=i32::MAX as f64).contains(&scaled.round())
            }
        }
    }
}

/// The ingest walk state.
struct Ingest<'a> {
    tree: &'a Tree,
    registry: ClipPathRegistry,
    scaler: Scaler,
    items: Vec<Item>,
    pattern_ids: HashMap<String, u32>,
    next_pattern_id: u32,
    /// No ordinary clip mutation has occurred (definitions may still be
    /// preserved).
    unused_clip_safe: bool,
    carried_clip: Option<usize>,
    active_selected_clip: Option<usize>,
    selected_clip: Option<usize>,
    interval_clip: Option<usize>,
    interval_stack: Vec<(Option<usize>, bool)>,
    /// The last drawn run's clip signature (bounds and canonical flag)
    /// and item index (the reuse candidate).
    scope_tail: Option<(([i64; 4], bool), usize)>,
    /// The last text run's font signature and item index.
    text_tail: Option<(TextSignature, usize)>,
    local_origin: [i64; 2],
}

/// The text font-state signature (font identity bytes, colour, align).
type TextSignature = (Vec<u8>, Color, u32);

/// The parsed root viewport.
#[derive(Clone, Copy, Debug, Default)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub view_box: Option<[f64; 4]>,
}

/// Ingests a parsed SVG tree into a VIM drawing.
///
/// # Errors
///
/// Fails on the reference translation's raised conditions (malformed
/// values, scaled-range overflow).
pub fn ingest(tree: &Tree, viewport: Viewport, scaler: Scaler) -> Result<Drawing, IngestFailure> {
    let registry = ClipPathRegistry::from_tree(tree);
    let Viewport {
        width,
        height,
        view_box,
    } = viewport;
    let mut ingest = Ingest {
        tree,
        registry,
        scaler,
        items: Vec::new(),
        pattern_ids: HashMap::new(),
        next_pattern_id: 1,
        unused_clip_safe: true,
        carried_clip: None,
        active_selected_clip: None,
        selected_clip: None,
        interval_clip: None,
        interval_stack: Vec::new(),
        scope_tail: None,
        text_tail: None,
        local_origin: [0, 0],
    };

    // The header facts: the translate group feeds the bounds; the legacy
    // broken-Y literal (with the integral-dimension conditions) marks the
    // degenerate-y repair whose canonical bounds the lowering emits.
    let header = header_group(tree);
    let legacy_literal = header.is_some_and(|group| {
        matches!(
            tree.kind(group),
            Kind::Group {
                legacy_broken_y: true,
                ..
            }
        )
    });
    let preamble = match header {
        Some(group) => {
            let Kind::Group { transform, .. } = tree.kind(group) else {
                unreachable!("header is a group")
            };
            Preamble {
                tx: transform.dx,
                ty: transform.dy,
            }
        }
        None => Preamble { tx: 0.0, ty: 0.0 },
    };
    let integral_over_two = [width, height]
        .iter()
        .all(|dimension| dimension.fract() == 0.0 && *dimension > 2.0);
    let degenerate = view_box.is_none() && legacy_literal && integral_over_two;
    let canvas = Canvas {
        width,
        height,
        preamble,
        degenerate_y_repair: degenerate,
        alias_namespace: false,
        scale: scaler.factor(),
    };

    // The mapping prelude rides on the canvas scale (the lowering emits
    // it); the walk starts at the header group's children when present.
    let roots: Vec<usize> = match header {
        Some(group) => tree.children(group).to_vec(),
        None => tree.children(tree.root).to_vec(),
    };
    for child in roots {
        ingest.dispatch(child)?;
    }
    Ok(Drawing {
        canvas,
        items: ingest.items,
    })
}

impl<'a> Ingest<'a> {
    // --- coordinate helpers ---

    fn to_int(&self, value: f64) -> Result<i64, IngestFailure> {
        self.scaler.to_int(value)
    }

    /// The attribute-hash point form: local origin subtracted, then scaled.
    fn point(&self, x: f64, y: f64) -> Result<DevicePoint, IngestFailure> {
        Ok(DevicePoint {
            x: self.to_int(x - self.local_origin[0] as f64)?,
            y: self.to_int(y - self.local_origin[1] as f64)?,
        })
    }

    fn rect(
        &self,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    ) -> Result<DeviceRect, IngestFailure> {
        Ok(DeviceRect {
            left: self.to_int(left - self.local_origin[0] as f64)?,
            top: self.to_int(top - self.local_origin[1] as f64)?,
            right: self.to_int(right - self.local_origin[0] as f64)?,
            bottom: self.to_int(bottom - self.local_origin[1] as f64)?,
        })
    }

    /// The size form: scaled without the local origin.
    fn size(&self, width: f64, height: f64) -> Result<DevicePoint, IngestFailure> {
        Ok(DevicePoint {
            x: self.to_int(width)?,
            y: self.to_int(height)?,
        })
    }

    fn emit(&mut self, item: Item) {
        self.items.push(item);
    }

    // --- dispatch ---

    fn dispatch(&mut self, id: usize) -> Result<(), IngestFailure> {
        let empty_group = self.registry.interval_group(id) && self.empty_clip();
        let clip = self.registry.interval_start(id);
        if let Some(clip) = clip {
            if (self.unused_clip_safe || self.interval_clip.is_some())
                && self.carried_clip.is_none()
                && self.active_selected_clip.is_none()
            {
                self.interval_stack
                    .push((self.interval_clip, self.unused_clip_safe));
                self.interval_clip = Some(clip);
                self.emit(Item::StatePush);
                if let Some(bounds) = unused_rectangle_bounds(self.tree, clip) {
                    let rect = self.rect(bounds[0], bounds[1], bounds[2], bounds[3])?;
                    self.emit(Item::ClipRectangle(rect));
                }
            }
        }
        self.run_handler(id)?;
        let ended = self
            .interval_clip
            .is_some_and(|interval| self.registry.interval_end(id) == Some(interval));
        if ended {
            self.finish_clip_interval();
        }
        if empty_group && self.empty_clip() {
            self.reset_empty_clip();
        }
        Ok(())
    }

    fn empty_clip(&self) -> bool {
        self.unused_clip_safe
            && self.interval_clip.is_none()
            && self.carried_clip.is_none()
            && self.active_selected_clip.is_none()
    }

    fn reset_empty_clip(&mut self) {
        self.emit(Item::ClipReset);
    }

    fn finish_clip_interval(&mut self) {
        self.emit(Item::StatePop);
        if self.active_selected_clip == self.interval_clip {
            self.active_selected_clip = None;
        }
        let (interval, safe) = self.interval_stack.pop().unwrap_or((None, true));
        self.interval_clip = interval;
        self.unused_clip_safe = safe;
        if self.interval_clip.is_none() {
            self.reset_empty_clip();
            self.unused_clip_safe = true;
        }
    }

    fn run_handler(&mut self, id: usize) -> Result<(), IngestFailure> {
        match tree_kind(self.tree, id) {
            Kind::Rect { .. } => self.draw_complete(id),
            Kind::Ellipse { .. } => self.draw_complete(id),
            Kind::Circle { .. } => self.draw_complete(id),
            Kind::Line { .. } => self.draw_complete(id),
            Kind::Polyline { .. } => self.draw_complete(id),
            Kind::Polygon { .. } => self.draw_complete(id),
            Kind::Path { .. } => self.draw_complete(id),
            Kind::Text { .. } => self.draw_complete(id),
            Kind::Image { .. } => self.draw_complete(id),
            Kind::Group { .. } => self.group(id),
            Kind::Defs { .. } => self.defs(id),
            Kind::ClipPath { .. } => Ok(()),
            Kind::Open { .. } => {
                for child in self.tree.children(id).to_vec() {
                    self.dispatch(child)?;
                }
                Ok(())
            }
        }
    }

    fn group(&mut self, id: usize) -> Result<(), IngestFailure> {
        let mut transform = match tree_kind(self.tree, id) {
            Kind::Group { transform, .. } => *transform,
            _ => Matrix::identity(),
        };
        // The wire transform carries the translation scaled (the 2x2
        // coefficients are ratios and survive unscaled).
        if self.scaler.scaled() {
            let factor = self.scaler.factor() as f64;
            transform.dx *= factor;
            transform.dy *= factor;
        }
        self.emit(Item::GroupOpen { transform });
        if let Some(clip) = self.registry.selected_clip(id) {
            if self.unused_clip_safe {
                self.selected_clip = Some(clip);
            }
        }
        for child in self.tree.children(id).to_vec() {
            self.dispatch(child)?;
        }
        self.emit(Item::GroupClose);
        if self.registry.selected_clip(id) == self.selected_clip {
            self.selected_clip = None;
            self.active_selected_clip = None;
        }
        let imported = self.registry.imported_clip(id);
        if self.carried_clip.is_some() && imported == self.carried_clip {
            self.reset_empty_clip();
            self.carried_clip = None;
        }
        let exported = self.registry.exported_clip(id);
        if let Some(clip) = exported {
            if self.unused_clip_safe {
                self.carried_clip = Some(clip);
                if let Some(bounds) = unused_rectangle_bounds(self.tree, clip) {
                    let rect = self.rect(bounds[0], bounds[1], bounds[2], bounds[3])?;
                    self.emit(Item::ClipRectangle(rect));
                }
            }
        }
        Ok(())
    }

    fn defs(&mut self, id: usize) -> Result<(), IngestFailure> {
        let (selected_pattern, unused_pattern) = match tree_kind(self.tree, id) {
            Kind::Defs {
                selected_pattern,
                unused_pattern,
                ..
            } => (selected_pattern.clone(), unused_pattern.clone()),
            _ => (None, None),
        };
        if let Some(pattern) = selected_pattern.or(unused_pattern) {
            self.emit_pattern(&pattern)?;
        }
        for child in self.tree.children(id).to_vec() {
            let clip = match tree_kind(self.tree, child) {
                Kind::ClipPath { .. } => child,
                _ => continue,
            };
            if self.preserve_selected_clip(clip) {
                continue;
            }
            if self.registry.unused_selected_clip(clip) && self.unused_clip_safe {
                self.emit(Item::ClipFromPath);
                self.reset_empty_clip();
                continue;
            }
            let Some(clip_id_value) = clip_id_of(self.tree, clip) else {
                continue;
            };
            if self.registry.referenced(&clip_id_value) {
                continue;
            }
            let Some(bounds) = unused_rectangle_bounds(self.tree, clip) else {
                continue;
            };
            if self.unused_clip_safe {
                let rect = self.rect(bounds[0], bounds[1], bounds[2], bounds[3])?;
                self.emit(Item::ClipRectangle(rect));
                self.reset_empty_clip();
            }
        }
        Ok(())
    }

    fn preserve_selected_clip(&mut self, clip: usize) -> bool {
        let selected_now = self.selected_clip == Some(clip);
        let interval_now = self.interval_clip == Some(clip)
            && match tree_kind(self.tree, clip) {
                Kind::ClipPath {
                    selected_path_data, ..
                } => selected_path_data.is_some(),
                _ => false,
            };
        if !(selected_now || interval_now) {
            return false;
        }
        self.emit(Item::ClipFromPath);
        if self.registry.unused_selected_clip(clip) && self.unused_clip_safe {
            self.reset_empty_clip();
        }
        if selected_now {
            self.active_selected_clip = Some(clip);
            self.selected_clip = None;
        }
        true
    }

    fn emit_pattern(
        &mut self,
        pattern: &crate::elements::CanonicalPattern,
    ) -> Result<(), IngestFailure> {
        let image = &pattern.image;
        let Some(href) = image.href.as_deref() else {
            return Ok(());
        };
        let Some(rest) = href.strip_prefix("data:image/png;base64,") else {
            return Ok(());
        };
        let Some(payload) = crate::elements::base64_decode(rest) else {
            return Ok(());
        };
        let dims = png_dims(&payload);
        if dims.map(|(w, h)| f64::from(w) == image.width && f64::from(h) == image.height)
            != Some(true)
        {
            return Ok(());
        }
        let id = self.next_pattern_id;
        self.next_pattern_id += 1;
        self.emit(Item::DefineRasterPattern(RasterPatternDef {
            id,
            raster: Raster {
                format: RasterFormat::Png,
                payload,
            },
            width: image.width as i64,
            height: image.height as i64,
        }));
        self.pattern_ids.insert(pattern.id.clone(), id);
        Ok(())
    }

    // --- the complete-run lifecycle ---

    /// Wraps one drawn element in its resolved run (the shared GDI
    /// lifecycle with its scope merge), then emits the drawn body.
    fn draw_complete(&mut self, id: usize) -> Result<(), IngestFailure> {
        let style = match tree_kind(self.tree, id) {
            Kind::Text { .. } => None,
            other => other.style().cloned(),
        };
        let clip_bounds = self.clip_bounds_for(id);
        let signature = self.clip_scope_signature(id, clip_bounds);
        let join = match (&signature, &self.scope_tail) {
            (Some(signature), Some((previous, index))) => {
                previous == signature && index + 1 == self.items.len() && self.complete_draw(id)
            }
            _ => false,
        };
        if join {
            // The previous run's bracket stays open; this run adds no
            // state records of its own.
            self.scope_tail = None;
        }
        let paint = self.run_paint(id, style.as_ref())?;
        let mut draw = self.run_body(id, &paint)?;
        draw.device_paint = Some(paint);
        if let (Some(bounds), false) = (clip_bounds, join) {
            draw.clip_bracket = Some(bounds);
        } else if join {
            draw.clip_join = true;
        }
        draw.provenance = self.run_provenance(id, &draw);
        let index = self.items.len();
        if let Some(signature) = &signature {
            if self.complete_draw(id) {
                self.scope_tail = Some((*signature, index));
            }
        }
        self.emit(Item::Draw(draw));
        Ok(())
    }

    fn complete_draw(&self, id: usize) -> bool {
        match tree_kind(self.tree, id) {
            Kind::Rect { .. }
            | Kind::Ellipse { .. }
            | Kind::Line { .. }
            | Kind::Polyline { .. }
            | Kind::Polygon { .. }
            | Kind::Path { .. } => {
                let style = tree_kind(self.tree, id).style();
                style.is_some() && style.is_some_and(|s| s.stroke.is_some() && true)
            }
            Kind::Image { .. } => true,
            Kind::Text { transform, .. } => matrix_identity(transform),
            _ => false,
        }
    }

    fn clip_bounds_for(&self, id: usize) -> Option<DeviceRect> {
        let attribute = tree_kind(self.tree, id).clip_path();
        let clip = self.registry.lookup(attribute)?;
        if Some(clip) == self.carried_clip
            || Some(clip) == self.selected_clip
            || self.active_selected_clip == Some(clip)
            || self.interval_clip == Some(clip)
        {
            return None;
        }
        let bounds = clip_bounds_of_resolved(self.tree, clip)?;
        let rect = self.rect(bounds[0], bounds[1], bounds[2], bounds[3]).ok()?;
        Some(rect)
    }

    fn clip_scope_signature(
        &self,
        id: usize,
        clip_bounds: Option<DeviceRect>,
    ) -> Option<([i64; 4], bool)> {
        let bounds = clip_bounds?;
        if !self.complete_draw(id) {
            return None;
        }
        let canonical = self.unused_clip_safe
            && self.carried_clip.is_none()
            && self.active_selected_clip.is_none()
            && tree_kind(self.tree, id)
                .clip_path()
                .and_then(|value| self.registry.lookup(Some(value)))
                .and_then(|clip| unused_rectangle_bounds(self.tree, clip))
                .is_some();
        Some((
            [bounds.left, bounds.top, bounds.right, bounds.bottom],
            canonical,
        ))
    }

    /// The run's device paint (pen, brush, miter) resolved from the style.
    fn run_paint(&self, id: usize, style: Option<&Style>) -> Result<DevicePaint, IngestFailure> {
        let Some(style) = style else {
            return Ok(DevicePaint {
                pen: None,
                brush: None,
                miter_limit: None,
            });
        };
        let stroke = style.stroke.as_ref().expect("styles carry strokes");
        // The miter limit rides on any painted stroke.
        let painted = !stroke.paint.null();
        let miter_limit = if painted {
            Some(self.serialized_miter_limit(id, stroke.miter_limit))
        } else {
            None
        };
        // The pen: width resolution per the provenance forms.
        let width = if let Some((stored, _)) = stroke.canonical_cosmetic_dash {
            DevicePoint {
                x: stored as i64,
                y: 0,
            }
        } else if stroke.cosmetic_pixel_width() {
            DevicePoint { x: 1, y: 0 }
        } else {
            self.point(stroke.width, 0.0)?
        };
        let color = match stroke.paint.color() {
            crate::color::SvgColor::Rgb { red, green, blue } => Color::new(red, green, blue),
            crate::color::SvgColor::Null => Color::new(0, 0, 0),
        };
        let pen = DevicePen {
            style: stroke.pen_style(),
            width,
            color,
        };
        // The brush: the fallback brush (the null-pen provenance form) or
        // the fill.
        let brush_paint = stroke
            .fallback_brush
            .clone()
            .unwrap_or_else(|| style.fill.clone());
        let brush = match &brush_paint {
            Paint::Pattern(reference) => {
                let _ = reference;
                None
            }
            Paint::Null => Some(DeviceBrush::Solid {
                style: 1,
                color: Color::new(0, 0, 0),
            }),
            Paint::Solid(color) => {
                let (red, green, blue) = color.channels();
                Some(DeviceBrush::Solid {
                    style: 0,
                    color: Color::new(red, green, blue),
                })
            }
        };
        // The pattern-reference brush is deferred to the body walk (it
        // selects the defined handle); carry it via the brush slot with a
        // resolved id reference.
        let brush = match (&brush_paint, &brush) {
            (Paint::Pattern(reference), _) => Some(DeviceBrush::Pattern {
                id: reference.clone(),
            }),
            (_, brush) => brush.clone(),
        };
        Ok(DevicePaint {
            pen: Some(pen),
            brush,
            miter_limit,
        })
    }

    /// The miter limit value in its wire form (the integer bit trick for
    /// the canonical polyline and cubic stroke paths).
    fn serialized_miter_limit(&self, id: usize, limit: f64) -> f32 {
        if let Kind::Path {
            line_record_kind,
            path_record_kind,
            commands,
            ..
        } = tree_kind(self.tree, id)
        {
            let cubic_figure = *path_record_kind == Some(PathKind::StrokePath)
                && commands.iter().any(|command| command.upcased() == "C");
            if (*line_record_kind == Some(LineKind::PolylineTo) || cubic_figure)
                && (1.0..=8_388_607.0).contains(&limit)
                && limit == limit.trunc()
            {
                // The integer payload rides the float bits verbatim.
                return f32::from_bits(limit as i64 as u32);
            }
        }
        limit as f32
    }

    /// The run's provenance payload, resolved per the reference order.
    fn run_provenance(&self, id: usize, _draw: &Draw) -> Option<Provenance> {
        let kind = tree_kind(self.tree, id);
        let clip = kind.clip_path().is_some();
        if let Kind::Rect {
            serialization_hint, ..
        } = kind
        {
            if let (Some(values), false) = (serialization_hint, clip) {
                return Some(rect_provenance(values));
            }
        }
        if let Some(style) = kind.style() {
            let stroke = style.stroke.as_ref()?;
            if stroke.canonical_cosmetic_dash.is_some() && stroke.raw_width == "1.0000" {
                if let crate::color::SvgColor::Rgb { red, green, blue } = stroke.paint.color() {
                    return Some(Provenance::StrokeColor(Color::new(red, green, blue)));
                }
            }
        }
        if let Kind::Image { href, .. } = kind {
            let rest = href.as_deref()?.strip_prefix("data:")?;
            let end = rest.find([';', ','])?;
            if &rest[..end] == "image/jpeg" {
                return Some(Provenance::Jpeg);
            }
        }
        None
    }

    // --- run bodies ---

    fn run_body(&mut self, id: usize, paint: &DevicePaint) -> Result<Draw, IngestFailure> {
        match tree_kind(self.tree, id).clone() {
            Kind::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
                style,
                ..
            } => self.rect_body(id, x, y, width, height, rx, ry, &style, paint),
            Kind::Ellipse { cx, cy, rx, ry, .. } => {
                let draw = Draw {
                    body: DrawBody::Shape(ShapeDraw {
                        kind: ShapeKind::Ellipse {
                            cx: self.unscaled(cx)?,
                            cy: self.unscaled(cy)?,
                            rx: self.unscaled(rx)?,
                            ry: self.unscaled(ry)?,
                        },
                    }),
                    ..Draw::default()
                };
                Ok(draw)
            }
            Kind::Circle { cx, cy, r, .. } => {
                let draw = Draw {
                    body: DrawBody::Shape(ShapeDraw {
                        kind: ShapeKind::Ellipse {
                            cx: self.unscaled(cx)?,
                            cy: self.unscaled(cy)?,
                            rx: self.unscaled(r)?,
                            ry: self.unscaled(r)?,
                        },
                    }),
                    ..Draw::default()
                };
                Ok(draw)
            }
            Kind::Line { x1, y1, x2, y2, .. } => {
                let start = self.point(x1, y1)?;
                let end = self.point(x2, y2)?;
                let mut geometry = PathGeometry::new(CoordJoin::Tuple);
                geometry.push(Segment::MoveTo {
                    x: start.x as f64,
                    y: start.y as f64,
                });
                geometry.push(Segment::LineTo {
                    x: end.x as f64,
                    y: end.y as f64,
                });
                Ok(Draw {
                    body: DrawBody::Path(PathDraw {
                        geometry,
                        program: Some(PathProgram::Segments),
                    }),
                    ..Draw::default()
                })
            }
            Kind::Polyline { points, .. } | Kind::Polygon { points, .. } => {
                let closed = matches!(tree_kind(self.tree, id), Kind::Polygon { .. });
                let mut geometry = PathGeometry::new(CoordJoin::Tuple);
                let device: Vec<DevicePoint> = points
                    .iter()
                    .map(|(x, y)| self.point(*x, *y))
                    .collect::<Result<_, _>>()?;
                for (index, point) in device.iter().enumerate() {
                    geometry.push(if index == 0 {
                        Segment::MoveTo {
                            x: point.x as f64,
                            y: point.y as f64,
                        }
                    } else {
                        Segment::LineTo {
                            x: point.x as f64,
                            y: point.y as f64,
                        }
                    });
                }
                Ok(Draw {
                    body: DrawBody::Path(PathDraw {
                        geometry,
                        program: if closed {
                            Some(PathProgram::Polygon)
                        } else {
                            Some(PathProgram::Polyline)
                        },
                    }),
                    ..Draw::default()
                })
            }
            Kind::Path { .. } => self.path_body(id, paint),
            Kind::Text { .. } => self.text_body(id),
            Kind::Image { .. } => self.image_body(id),
            _ => Ok(Draw::default()),
        }
    }

    /// Shapes carry resolved device geometry directly (no local origins).
    fn unscaled(&self, value: f64) -> Result<f64, IngestFailure> {
        Ok(self.to_int(value)? as f64)
    }

    #[allow(clippy::too_many_arguments)]
    fn rect_body(
        &self,
        _id: usize,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: f64,
        ry: f64,
        style: &Style,
        _paint: &DevicePaint,
    ) -> Result<Draw, IngestFailure> {
        let kind = if rx > 0.0 || ry > 0.0 {
            ShapeKind::RoundedRect {
                x: self.unscaled(x)?,
                y: self.unscaled(y)?,
                width: self.unscaled(width)?,
                height: self.unscaled(height)?,
                rx: self.unscaled(rx)?,
                ry: self.unscaled(ry)?,
            }
        } else {
            ShapeKind::Rect {
                x: self.unscaled(x)?,
                y: self.unscaled(y)?,
                width: self.unscaled(width)?,
                height: self.unscaled(height)?,
            }
        };
        let _raster_fill = rx <= 0.0
            && ry <= 0.0
            && style.clip_path.is_none()
            && style
                .stroke
                .as_ref()
                .is_some_and(|stroke| stroke.paint.null())
            && matches!(style.fill, Paint::Solid(_));
        Ok(Draw {
            body: DrawBody::Shape(ShapeDraw { kind }),
            arrangement: Arrangement::RegionFillFirst,
            ..Draw::default()
        })
    }

    fn path_body(&self, id: usize, _paint: &DevicePaint) -> Result<Draw, IngestFailure> {
        let (commands, style, line_kind, path_kind) = match tree_kind(self.tree, id) {
            Kind::Path {
                commands,
                style,
                line_record_kind,
                path_record_kind,
            } => (
                commands.clone(),
                style.clone(),
                *line_record_kind,
                *path_record_kind,
            ),
            _ => unreachable!("path body"),
        };
        if commands.is_empty() && path_kind != Some(PathKind::StrokePath) {
            return Ok(Draw::default());
        }
        // The native circular arc recoveries.
        if matches!(
            path_kind,
            Some(PathKind::Arc) | Some(PathKind::Chord) | Some(PathKind::Pie)
        ) {
            let family = match path_kind {
                Some(PathKind::Arc) => crate::arcs::ArcFamily::Arc,
                Some(PathKind::Chord) => crate::arcs::ArcFamily::Chord,
                _ => crate::arcs::ArcFamily::Pie,
            };
            let mut arc = crate::arcs::from_commands(&commands, family);
            if arc.is_none() && family == crate::arcs::ArcFamily::Arc && self.scaler.scaled() {
                arc = crate::arcs::mapped_arc(&commands, self.scaler.factor());
            }
            if arc.is_none() && family == crate::arcs::ArcFamily::Pie && self.scaler.scaled() {
                arc = crate::arcs::mapped_pie(&commands, self.scaler.factor());
            }
            if let Some(arc) = arc {
                if self.arc_safe(&arc) {
                    return self.arc_draw(&arc);
                }
            }
        }
        let preserve_initial_line = path_kind == Some(PathKind::StrokePath);
        let subpaths = flattener::flatten(&commands, preserve_initial_line);
        if path_kind == Some(PathKind::BitBlt) {
            return self.blit_body(subpaths.first());
        }
        if path_kind == Some(PathKind::PolyBezierTo) {
            return self.current_curve_body(&subpaths);
        }
        if line_kind == Some(LineKind::PolylineTo) {
            return self.current_polyline_body(subpaths.first());
        }
        if matches!(
            path_kind,
            Some(PathKind::EndPath)
                | Some(PathKind::FillPath)
                | Some(PathKind::StrokePath)
                | Some(PathKind::StrokeAndFillPath)
        ) {
            let stroke_null = style
                .stroke
                .as_ref()
                .map(|stroke| stroke.paint.null())
                .unwrap_or(true);
            let unfilled = matches!(style.fill, Paint::Null);
            return self.figures_body(
                &subpaths,
                stroke_null,
                unfilled,
                path_kind == Some(PathKind::EndPath),
            );
        }
        self.subpaths_body(&subpaths, line_kind, path_kind, &style)
    }

    fn arc_safe(&self, arc: &crate::arcs::ResolvedArc) -> bool {
        let (x, y) = arc.center;
        let values = [
            x - arc.radius,
            y - arc.radius,
            x + arc.radius,
            y + arc.radius,
        ];
        let factor = self.scaler.factor() as f64;
        values
            .iter()
            .all(|value| ((value * factor).round() as i64).abs() <= i32::MAX as i64)
    }

    fn arc_draw(&self, arc: &crate::arcs::ResolvedArc) -> Result<Draw, IngestFailure> {
        let with_origin = arc.origin.filter(|_| self.scaler.scaled());
        // The mapped family's coordinates carry this origin.
        let origin_shift = with_origin
            .map(|value| [value.0 as i64, value.1 as i64])
            .unwrap_or(self.local_origin);
        let point = |x: f64, y: f64| -> Result<DevicePoint, IngestFailure> {
            Ok(DevicePoint {
                x: self.to_int(x - origin_shift[0] as f64)?,
                y: self.to_int(y - origin_shift[1] as f64)?,
            })
        };
        let rect =
            |left: f64, top: f64, right: f64, bottom: f64| -> Result<DeviceRect, IngestFailure> {
                Ok(DeviceRect {
                    left: self.to_int(left - origin_shift[0] as f64)?,
                    top: self.to_int(top - origin_shift[1] as f64)?,
                    right: self.to_int(right - origin_shift[0] as f64)?,
                    bottom: self.to_int(bottom - origin_shift[1] as f64)?,
                })
            };
        let prefix = point(arc.prefix.0, arc.prefix.1)?;
        let finish = point(arc.finish.0, arc.finish.1)?;
        let box_rect = rect(
            arc.center.0 - arc.radius,
            arc.center.1 - arc.radius,
            arc.center.0 + arc.radius,
            arc.center.1 + arc.radius,
        )?;
        let start = point(
            arc.radials[0].0 + arc.center.0,
            arc.radials[0].1 + arc.center.1,
        )?;
        let end = point(
            arc.radials[1].0 + arc.center.0,
            arc.radials[1].1 + arc.center.1,
        )?;
        let kind = match arc.kind {
            crate::arcs::ArcFamily::Arc => ArcKind::Arc,
            crate::arcs::ArcFamily::Chord => ArcKind::Chord,
            crate::arcs::ArcFamily::Pie => ArcKind::Pie,
        };
        let origin = with_origin.map(|value| DevicePoint {
            x: value.0 as i64,
            y: value.1 as i64,
        });
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry: PathGeometry::new(CoordJoin::Tuple),
                program: Some(PathProgram::Circular(ArcForm {
                    kind,
                    clockwise: arc.sweep,
                    prefix,
                    finish,
                    arc_box: box_rect,
                    start,
                    end,
                })),
            }),
            origin,
            ..Draw::default()
        })
    }

    fn blit_body(&self, subpath: Option<&SubPath>) -> Result<Draw, IngestFailure> {
        let Some(subpath) = subpath else {
            return Ok(Draw::default());
        };
        let Some(&(left, top)) = subpath.points.first() else {
            return Ok(Draw::default());
        };
        let Some(&(right, bottom)) = subpath.points.get(2) else {
            return Ok(Draw::default());
        };
        let _dest = self.point(left, top)?;
        let extent = self.size(right - left, bottom - top)?;
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for (x, y) in &subpath.points {
            let point = self.point(*x, *y)?;
            geometry.push(Segment::LineTo {
                x: point.x as f64,
                y: point.y as f64,
            });
        }
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::Blit { extent }),
            }),
            ..Draw::default()
        })
    }

    fn current_curve_body(&self, subpaths: &[SubPath]) -> Result<Draw, IngestFailure> {
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for subpath in subpaths {
            let Some(&(first_x, first_y)) = subpath.points.first() else {
                continue;
            };
            let first = self.point(first_x, first_y)?;
            geometry.push(Segment::MoveTo {
                x: first.x as f64,
                y: first.y as f64,
            });
            // Cubic runs carry their control points as CubicTo segments.
            for (letter, points) in &subpath.segments {
                if letter != "C" {
                    continue;
                }
                push_cubic(&mut geometry, points, self)?;
            }
        }
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::CurvesPerContour),
            }),
            ..Draw::default()
        })
    }

    fn current_polyline_body(&self, subpath: Option<&SubPath>) -> Result<Draw, IngestFailure> {
        let Some(subpath) = subpath else {
            return Ok(Draw::default());
        };
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for (index, (x, y)) in subpath.points.iter().enumerate() {
            let point = self.point(*x, *y)?;
            geometry.push(if index == 0 {
                Segment::MoveTo {
                    x: point.x as f64,
                    y: point.y as f64,
                }
            } else {
                Segment::LineTo {
                    x: point.x as f64,
                    y: point.y as f64,
                }
            });
        }
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::LineFromCurrent),
            }),
            ..Draw::default()
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn figures_body(
        &self,
        subpaths: &[SubPath],
        stroke_null: bool,
        unfilled: bool,
        pending: bool,
    ) -> Result<Draw, IngestFailure> {
        let closed_regions = pending
            && subpaths.len() > 1
            && subpaths
                .iter()
                .all(|subpath| subpath.shape_kind == flattener::ShapeKind::Polygon);
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for subpath in subpaths {
            for (letter, points) in &subpath.segments {
                match letter.as_str() {
                    "M" => {
                        let point = self.point(points[0].0, points[0].1)?;
                        geometry.push(Segment::MoveTo {
                            x: point.x as f64,
                            y: point.y as f64,
                        });
                    }
                    "L" | "H" | "V" => {
                        let point = self.point(points[0].0, points[0].1)?;
                        geometry.push(Segment::LineTo {
                            x: point.x as f64,
                            y: point.y as f64,
                        });
                    }
                    "C" => push_cubic(&mut geometry, points, self)?,
                    "Z" => geometry.push(Segment::Close),
                    _ => {}
                }
            }
        }
        let paint = if pending {
            None
        } else if stroke_null && !unfilled {
            Some(FigurePaint::Fill)
        } else if unfilled && !stroke_null {
            Some(FigurePaint::Stroke)
        } else {
            Some(FigurePaint::StrokeFill)
        };
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::Figure {
                    paint: paint.unwrap_or(FigurePaint::StrokeFill),
                    pending,
                    closed_regions,
                }),
            }),
            ..Draw::default()
        })
    }

    fn subpaths_body(
        &self,
        subpaths: &[SubPath],
        line_kind: Option<LineKind>,
        path_kind: Option<PathKind>,
        style: &Style,
    ) -> Result<Draw, IngestFailure> {
        let unfilled = matches!(style.fill, Paint::Null);
        let stroke_null = style
            .stroke
            .as_ref()
            .map(|stroke| stroke.paint.null())
            .unwrap_or(true);
        let kinds: Vec<flattener::ShapeKind> = subpaths.iter().map(|s| s.shape_kind).collect();
        let unique: Option<flattener::ShapeKind> = first_unique(&kinds);
        if unfilled && prefixed_polybezier(subpaths) {
            let prefix = subpaths[0].points.first().copied();
            let last = subpaths.last().expect("checked nonempty");
            return self.polybezier_prefixed(prefix, last);
        }
        let multi = matches!(
            unique,
            Some(flattener::ShapeKind::Polygon) | Some(flattener::ShapeKind::Polyline)
        );
        if subpaths.len() > 1 && multi {
            return self.multi_body(subpaths, unique == Some(flattener::ShapeKind::Polygon));
        }
        if path_kind == Some(PathKind::PolyPolygon)
            && subpaths.len() == 1
            && unique == Some(flattener::ShapeKind::Polygon)
        {
            return self.multi_body(subpaths, true);
        }
        if line_kind != Some(LineKind::Polyline) && single_line_path(subpaths) {
            let mut geometry = PathGeometry::new(CoordJoin::Tuple);
            for (letter, points) in &subpaths[0].segments {
                let point = self.point(points[0].0, points[0].1)?;
                geometry.push(if letter == "M" {
                    Segment::MoveTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    }
                } else {
                    Segment::LineTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    }
                });
            }
            return Ok(Draw {
                body: DrawBody::Path(PathDraw {
                    geometry,
                    program: Some(PathProgram::Segments),
                }),
                ..Draw::default()
            });
        }
        if subpaths.len() > 1 || unique == Some(flattener::ShapeKind::Path) {
            return self.figures_body(subpaths, stroke_null, unfilled, false);
        }
        let Some(flattened) = subpaths.first() else {
            return Ok(Draw::default());
        };
        match flattened.shape_kind {
            flattener::ShapeKind::Polygon => self.single_poly(flattened, true),
            flattener::ShapeKind::Polyline => self.single_poly(flattened, false),
            flattener::ShapeKind::PolyBezier => {
                let prefix = flattened.points.first().copied();
                self.polybezier_prefixed(prefix, flattened)
            }
            flattener::ShapeKind::Path => Ok(Draw::default()),
        }
    }

    fn single_poly(&self, subpath: &SubPath, closed: bool) -> Result<Draw, IngestFailure> {
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for (index, (x, y)) in subpath.points.iter().enumerate() {
            let point = self.point(*x, *y)?;
            geometry.push(if index == 0 {
                Segment::MoveTo {
                    x: point.x as f64,
                    y: point.y as f64,
                }
            } else {
                Segment::LineTo {
                    x: point.x as f64,
                    y: point.y as f64,
                }
            });
        }
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: if closed {
                    Some(PathProgram::Polygon)
                } else {
                    Some(PathProgram::Polyline)
                },
            }),
            ..Draw::default()
        })
    }

    fn polybezier_prefixed(
        &self,
        prefix: Option<(f64, f64)>,
        subpath: &SubPath,
    ) -> Result<Draw, IngestFailure> {
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        let mut first = true;
        for (letter, points) in &subpath.segments {
            match letter.as_str() {
                "M" => {
                    let point = self.point(points[0].0, points[0].1)?;
                    geometry.push(Segment::MoveTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    });
                }
                "C" => push_cubic(&mut geometry, points, self)?,
                "L" => {
                    let point = self.point(points[0].0, points[0].1)?;
                    geometry.push(Segment::LineTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    });
                }
                _ => {}
            }
            let _ = &mut first;
        }
        let prefix_point = match prefix {
            Some((x, y)) => Some(self.point(x, y)?),
            None => None,
        };
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::PolyBezier {
                    prefix: prefix_point,
                }),
            }),
            ..Draw::default()
        })
    }

    fn multi_body(&self, subpaths: &[SubPath], closed: bool) -> Result<Draw, IngestFailure> {
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        for subpath in subpaths {
            for (index, (x, y)) in subpath.points.iter().enumerate() {
                let point = self.point(*x, *y)?;
                geometry.push(if index == 0 {
                    Segment::MoveTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    }
                } else {
                    Segment::LineTo {
                        x: point.x as f64,
                        y: point.y as f64,
                    }
                });
            }
        }
        Ok(Draw {
            body: DrawBody::Path(PathDraw {
                geometry,
                program: Some(PathProgram::MultiContour { closed }),
            }),
            ..Draw::default()
        })
    }

    // --- text runs ---

    fn text_body(&mut self, id: usize) -> Result<Draw, IngestFailure> {
        let Kind::Text {
            x,
            y,
            content,
            font_family,
            font_size,
            font_weight,
            italic,
            text_anchor,
            style,
            transform,
            empty_cdata_content,
            canonical_rotation,
        } = tree_kind(self.tree, id).clone()
        else {
            unreachable!("text body");
        };
        if content.is_empty() && !empty_cdata_content && font_size != 0.0 {
            // The reference skips empty runs unless the CDATA provenance
            // or a zero font size says otherwise.
            return Ok(Draw::default());
        }
        let transformed = !matrix_identity(&transform);
        let _ = transformed;
        let _clip = self.clip_bounds_for_text(id);
        let rotation = canonical_rotation;
        // The effective text scaler: the finer native grid when the
        // recovery carried a native height and every value survives it.
        let mut effective = self.scaler;
        if let Some(rotation) = &rotation {
            if rotation.native_height.is_some() && self.scaler.scaled() {
                let finer = Scaler::Fixed(1_000_000);
                let values = [
                    x,
                    rotation.native_y.unwrap_or(y),
                    rotation.native_height.unwrap_or(font_size),
                    x + (content.chars().count() as f64 * font_size * 0.6),
                    y - font_size,
                ];
                if values.iter().all(|value| !finer.unsafe_value(*value)) {
                    effective = finer;
                }
            }
        }
        let scale_override = (effective != self.scaler).then(|| effective.factor());
        let to_int = |value: f64| effective.to_int(value);
        // The reference point.
        let reference_y = if let Some(rotation) = &rotation {
            if rotation.native_y.is_some() && effective.scaled() && effective.factor() == 1_000_000
            {
                rotation.native_y.unwrap_or(y)
            } else if rotation.vertical_alignment == crate::text::Alignment::Top {
                y - (font_size * 0.9)
            } else {
                y
            }
        } else {
            y
        };
        let reference = DevicePoint {
            x: to_int(x)?,
            y: to_int(reference_y)?,
        };
        // The declared bounds.
        let width = content.chars().count() as f64 * font_size * 0.6;
        let bounds = DeviceRect {
            left: to_int(x)?,
            top: to_int(y - font_size)?,
            right: to_int(x + width)?,
            bottom: to_int(y)?,
        };
        // The LOGFONT identity bytes.
        let font_height = if effective.factor() == 1_000_000 {
            rotation
                .as_ref()
                .and_then(|rotation| rotation.native_height)
                .unwrap_or(font_size)
        } else {
            font_size
        };
        let escapement = rotation
            .as_ref()
            .map(|rotation| rotation.escapement)
            .unwrap_or(0);
        let weight = if font_weight == 0 { 0 } else { font_weight };
        let face_utf16: Vec<u16> = font_family
            .as_deref()
            .unwrap_or("")
            .encode_utf16()
            .collect();
        let mut logfont = Vec::with_capacity(92);
        push_i32(&mut logfont, to_int(font_height)? as i32);
        push_i32(&mut logfont, 0);
        push_i32(&mut logfont, escapement);
        push_i32(&mut logfont, 0);
        push_i32(&mut logfont, weight as i32);
        logfont.push(u8::from(italic));
        logfont.push(0);
        logfont.push(0);
        logfont.push(0);
        logfont.push(0);
        logfont.push(0);
        logfont.push(0);
        logfont.push(0);
        for index in 0..32 {
            let unit = face_utf16.get(index).copied().unwrap_or(0);
            logfont.extend_from_slice(&unit.to_le_bytes());
        }
        let vertical = match rotation
            .as_ref()
            .map(|rotation| rotation.vertical_alignment)
        {
            Some(crate::text::Alignment::Top) => 0,
            _ => 0x18,
        };
        let horizontal = match text_anchor.as_str() {
            "middle" => 0x06,
            "end" => 0x02,
            _ => 0,
        };
        let align = vertical | horizontal;
        let color = match style.fill.color() {
            crate::color::SvgColor::Rgb { red, green, blue } => Color::new(red, green, blue),
            crate::color::SvgColor::Null => Color::new(0, 0, 0),
        };
        // The font-state signature and its adjacency reuse.
        let signature: TextSignature = (logfont.clone(), color, align);
        let continuation = self.text_tail.as_ref().is_some_and(|(previous, index)| {
            *previous == signature && index + 1 == self.items.len()
        });
        if continuation {
            self.text_tail = None;
        }
        let text = TextDraw {
            x: to_int(x)? as f64,
            y: to_int(y)? as f64,
            font_height: to_int(font_height)? as f64,
            face: font_family,
            color,
            rotation: rotation.as_ref().map(|rotation| rotation.angle),
            italic,
            underline: false,
            strikeout: false,
            weight,
            anchor: match text_anchor.as_str() {
                "middle" => TextAnchor::Middle,
                "end" => TextAnchor::End,
                _ => TextAnchor::Start,
            },
            baseline: match rotation
                .as_ref()
                .map(|rotation| rotation.vertical_alignment)
            {
                Some(crate::text::Alignment::Top) => eikogram::BaselineMode::Raised,
                _ => eikogram::BaselineMode::AtY,
            },
            content: content.clone(),
            native: rotation.as_ref().map(|rotation| TextNative {
                escapement: rotation.escapement,
                height: rotation.native_height,
                y: rotation.native_y,
            }),
            font_continuation: continuation,
            device_run: Some(TextRun {
                bounds,
                reference,
                font: logfont,
                align,
            }),
        };
        let draw = Draw {
            body: DrawBody::Text(text),
            scale: scale_override,
            world_transform: transformed.then_some(transform),
            ..Draw::default()
        };
        let index = self.items.len();
        self.text_tail = Some((signature, index));
        Ok(draw)
    }

    fn clip_bounds_for_text(&self, id: usize) -> Option<DeviceRect> {
        self.clip_bounds_for(id)
    }

    // --- image runs ---

    fn image_body(&self, id: usize) -> Result<Draw, IngestFailure> {
        let Kind::Image {
            x,
            y,
            width,
            height,
            href,
            ..
        } = tree_kind(self.tree, id).clone()
        else {
            unreachable!("image body");
        };
        let Some(href) = href else {
            return Ok(Draw::default());
        };
        let rest = match href.strip_prefix("data:") {
            Some(rest) => rest,
            None => return Ok(Draw::default()),
        };
        let Some(media_end) = rest.find([';', ',']) else {
            return Ok(Draw::default());
        };
        let media = &rest[..media_end];
        if !matches!(media, "image/png" | "image/jpeg" | "image/jpg") {
            return Ok(Draw::default());
        }
        let Some(comma) = rest.find(',') else {
            return Ok(Draw::default());
        };
        let payload = &rest[comma + 1..];
        let decoded = if href.contains(";base64,") {
            crate::elements::base64_decode(payload).unwrap_or_default()
        } else {
            payload.as_bytes().to_vec()
        };
        let (format, payload) = if media == "image/png" {
            (RasterFormat::Png, decoded)
        } else {
            (RasterFormat::Jpeg { zero_padded: false }, decoded)
        };
        let bounds = self.rect(x, y, x + width, y + height)?;
        let destination = DevicePoint {
            x: self.to_int(x)?,
            y: self.to_int(y)?,
        };
        let extent = self.size(width, height)?;
        Ok(Draw {
            body: DrawBody::Image(ImageDraw {
                raster: Raster { format, payload },
                arrangement: ImageArrangement::Device {
                    bounds,
                    destination,
                    extent,
                },
            }),
            ..Draw::default()
        })
    }
}

fn tree_kind(tree: &Tree, id: usize) -> &Kind {
    &tree.elements[id].kind
}

fn clip_id_of(tree: &Tree, id: usize) -> Option<String> {
    match tree_kind(tree, id) {
        Kind::ClipPath { clip_id, .. } => clip_id.clone(),
        _ => None,
    }
}

fn clip_bounds_of_resolved(tree: &Tree, clip: usize) -> Option<[f64; 4]> {
    match tree_kind(tree, clip) {
        Kind::ClipPath { clip_bounds, .. } => *clip_bounds,
        _ => None,
    }
}

fn matrix_identity(matrix: &Matrix) -> bool {
    *matrix == Matrix::identity()
}

fn rect_provenance(values: &[String; 5]) -> Provenance {
    Provenance::RectText(eikogram::BareRect {
        x: values[0].clone(),
        y: values[1].clone(),
        width: values[2].clone(),
        height: values[3].clone(),
        fill: values[4].clone(),
    })
}

fn push_cubic(
    geometry: &mut PathGeometry,
    points: &[(f64, f64)],
    ingest: &Ingest<'_>,
) -> Result<(), IngestFailure> {
    if points.len() < 6 {
        return Ok(());
    }
    let mut device = Vec::new();
    for (x, y) in points.iter().take(6) {
        device.push(ingest.point(*x, *y)?);
    }
    geometry.push(Segment::CubicTo {
        x1: device[0].x as f64,
        y1: device[0].y as f64,
        x2: device[1].x as f64,
        y2: device[1].y as f64,
        x: device[2].x as f64,
        y: device[2].y as f64,
    });
    Ok(())
}

fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn first_unique<T: PartialEq + Copy>(values: &[T]) -> Option<T> {
    let mut unique: Vec<T> = Vec::new();
    for value in values {
        if !unique.contains(value) {
            unique.push(*value);
        }
    }
    if unique.len() == 1 {
        Some(unique[0])
    } else {
        None
    }
}

fn prefixed_polybezier(subpaths: &[SubPath]) -> bool {
    subpaths.len() == 2
        && subpaths[0]
            .segments
            .iter()
            .map(|(letter, _)| letter.as_str())
            .collect::<Vec<&str>>()
            == ["M"]
        && subpaths[1].shape_kind == flattener::ShapeKind::PolyBezier
}

fn single_line_path(subpaths: &[SubPath]) -> bool {
    if subpaths.len() != 1 {
        return false;
    }
    let segments = &subpaths[0].segments;
    segments.len() == 2 && segments[0].0 == "M" && matches!(segments[1].0.as_str(), "L" | "H" | "V")
}

/// The brush-fallback stroke: the carried paint at the raw one-pixel form
/// with every other fact at its default (the reference constructor).
pub fn stroke_for_fallback(paint: Paint) -> crate::paint::Stroke {
    let mut stroke = crate::paint::Stroke::parse(
        Some("#000000"),
        Some("1px"),
        None,
        None,
        None,
        None,
        &Paint::Null,
    )
    .expect("literal defaults parse");
    stroke.paint = paint.clone();
    stroke.width = 1.0;
    stroke.raw_width = "1px".to_string();
    stroke.line_cap = "round".to_string();
    stroke.line_join = "round".to_string();
    stroke.miter_limit = 10.0;
    stroke.dash_array = None;
    stroke.line_cap_explicit = false;
    stroke.line_join_explicit = false;
    stroke.null_pen_fallback = true;
    stroke.fallback_brush = Some(paint);
    stroke
}

/// The pixel dimensions of a PNG payload (its IHDR), for the pattern
/// dimension validation.
fn png_dims(payload: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if payload.len() < 24 || payload[..8] != SIGNATURE || &payload[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(payload[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(payload[20..24].try_into().ok()?);
    Some((width, height))
}
