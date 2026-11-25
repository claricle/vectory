//! The record walker: the reference renderer's visit semantics, lifting the
//! emf-core model into VIM items. Every coordinate leaves this module in
//! output space; every paint leaves it resolved.

use std::collections::HashMap;

use eikogram::{
    Arrangement, BaselineMode, Canvas, ClipDef, ClipShape, Color, CoordJoin, Dash, Drawing, Draw,
    DrawBody, Fill, FillStyle, HatchStyle, HatchPatternDef, ImageArrangement, ImageDraw, Item,
    LineCap, LineJoin, Matrix, PathDraw, PathGeometry, PathPaint, Preamble, Raster, RasterFormat,
    RasterPatternDef, Segment, ShapeDraw, ShapeKind, Stroke, TextAnchor, TextDraw, Width,
};

use crate::analysis::{self, PathAction, PathAnalysis, PathTransform};
use crate::fingerprint::sha256_hex;
use crate::glyph;
use crate::hint::{self, DrawingHint};
use crate::raster;
use crate::state::{BsdRand, Brush, DeviceContext, Font, GdiObject, ObjectTable, Pen, SaveStack};

/// Render options (the reference renderer's keyword options).
#[derive(Clone, Debug, Default)]
pub struct RenderOptions {
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// When set to the SVG alias, the canvas namespace is declared twice.
    pub namespace: Option<String>,
}

const MAX_RECORDS: u64 = 1_000_000;

/// Renders an EMF metafile model into a VIM drawing.
///
/// # Errors
///
/// Fails on header/output dimension validation (the reference renderer's
/// raised conditions) or an over-limit record count.
pub fn render(
    document: &emf_core::Document,
    options: &RenderOptions,
) -> Result<Drawing, crate::RenderFailure> {
    let header = document.header();
    let bounds = header.bounds;
    let fix_broken_y =
        (bounds.top < 0 && bounds.bottom > 0) || (bounds.top > 0 && bounds.bottom < 0);

    let (img_width, img_height) = compute_output_dimensions(bounds, options)?;

    let bounds_width = i64::from(bounds.right - bounds.left).abs();
    let scaling = if bounds_width == 0 { 1.0 } else { img_width / bounds_width as f64 };
    let ref_x = f64::from(bounds.left);
    let ref_y = f64::from(bounds.top);

    let device = header.device;
    let mm = header.millimeters;
    let px_per_mm = if mm.cx == 0 { 1.0 } else { f64::from(device.cx) / f64::from(mm.cx) };

    let records = document.records();
    if records.len() as u64 > MAX_RECORDS {
        return Err(crate::RenderFailure::ResourceLimit { limit: MAX_RECORDS });
    }

    let analysis_result = analysis::analyze(records);

    let mut walker = Walker {
        canvas: Canvas {
            width: img_width,
            height: img_height,
            preamble: Preamble { tx: -ref_x * scaling, ty: -ref_y * scaling },
            degenerate_y_repair: fix_broken_y,
            alias_namespace: options.namespace.as_deref() == Some("svg"),
        },
        fix_broken_y,
        scaling,
        px_per_mm,
        dc: DeviceContext::default(),
        object_table: ObjectTable::default(),
        save_stack: SaveStack::default(),
        bsd_rand: BsdRand::default(),
        analysis: analysis_result,
        current_x: 0.0,
        current_y: 0.0,
        open_groups: 0,
        in_path: false,
        pending_action: PathAction::End,
        current_begin: None,
        building: None,
        last_path_geometry: None,
        items: Vec::new(),
        image_library: HashMap::new(),
        next_image_id: 1,
        hatch_counter: 0,
        pending_hint: None,
        hint: None,
    };
    walker.walk(document, records);
    walker.finish();
    Ok(Drawing { canvas: walker.canvas, items: walker.items })
}

fn compute_output_dimensions(
    bounds: emf_core::Rect,
    options: &RenderOptions,
) -> Result<(f64, f64), crate::RenderFailure> {
    let ratio_xy = f64::from(bounds.right - bounds.left) / f64::from(bounds.bottom - bounds.top);
    let mut img_width;
    let mut img_height;

    if let (Some(w), Some(h)) = (options.width, options.height) {
        img_width = w;
        img_height = h;
        let tmp_w = img_height * ratio_xy;
        if tmp_w > img_width {
            img_height = img_width / ratio_xy;
        } else {
            img_width = tmp_w;
        }
    } else if let Some(h) = options.height {
        img_height = h;
        img_width = img_height * ratio_xy;
    } else if let Some(w) = options.width {
        img_width = w;
        img_height = img_width / ratio_xy;
    } else {
        img_width = f64::from(bounds.right - bounds.left).abs();
        img_height = f64::from(bounds.bottom - bounds.top).abs();
    }

    for (value, label) in [(img_width, "Output width"), (img_height, "Output height")] {
        if !value.is_finite() || value <= 0.0 {
            return Err(crate::RenderFailure::Dimension(format!(
                "{label} must be finite and positive"
            )));
        }
    }
    Ok((img_width, img_height))
}

/// The wire matrix as f64 (the reference computes in Ruby floats).
#[derive(Clone, Copy, Debug)]
struct MatrixF64 {
    m11: f64,
    m12: f64,
    m21: f64,
    m22: f64,
    dx: f64,
    dy: f64,
}

fn matrix_of(xform: &emf_core::XForm) -> MatrixF64 {
    MatrixF64 {
        m11: f64::from(xform.m11.to_f32()),
        m12: f64::from(xform.m12.to_f32()),
        m21: f64::from(xform.m21.to_f32()),
        m22: f64::from(xform.m22.to_f32()),
        dx: f64::from(xform.dx.to_f32()),
        dy: f64::from(xform.dy.to_f32()),
    }
}

fn identity() -> MatrixF64 {
    MatrixF64 { m11: 1.0, m12: 0.0, m21: 0.0, m22: 1.0, dx: 0.0, dy: 0.0 }
}

fn multiply(a: &MatrixF64, b: &MatrixF64) -> MatrixF64 {
    MatrixF64 {
        m11: a.m11 * b.m11 + a.m12 * b.m21,
        m12: a.m11 * b.m12 + a.m12 * b.m22,
        m21: a.m21 * b.m11 + a.m22 * b.m21,
        m22: a.m21 * b.m12 + a.m22 * b.m22,
        dx: a.dx * b.m11 + a.dy * b.m21 + b.dx,
        dy: a.dx * b.m12 + a.dy * b.m22 + b.dy,
    }
}

struct Walker {
    canvas: Canvas,
    fix_broken_y: bool,
    scaling: f64,
    px_per_mm: f64,
    dc: DeviceContext,
    object_table: ObjectTable,
    save_stack: SaveStack,
    bsd_rand: BsdRand,
    analysis: PathAnalysis,
    current_x: f64,
    current_y: f64,
    open_groups: u32,
    in_path: bool,
    pending_action: PathAction,
    current_begin: Option<usize>,
    building: Option<PathGeometry>,
    last_path_geometry: Option<PathGeometry>,
    items: Vec<Item>,
    image_library: HashMap<Vec<u8>, u32>,
    next_image_id: u32,
    hatch_counter: u32,
    pending_hint: Option<DrawingHint>,
    hint: Option<DrawingHint>,
}

impl Walker {
    // --- coordinate mapping (the reference point_cal) ---

    fn use_orgs(&self) -> bool {
        let mode = self.dc.coordinate_state.borrow().map_mode;
        mode == 7 || mode == 8
    }

    fn sf_x(&self) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        match coord.map_mode {
            1 => 1.0,
            2 => self.px_per_mm * 0.1,
            3 => self.px_per_mm * 0.01,
            4 => self.px_per_mm * 0.01 * 25.4,
            5 => self.px_per_mm * 0.001 * 25.4,
            6 => self.px_per_mm / 1440.0 * 25.4,
            7 | 8 => {
                if coord.window_ext_set && coord.viewport_ext_set {
                    f64::from(coord.viewport_ext.0) / f64::from(coord.window_ext.0)
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    fn sf_y(&self) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        match coord.map_mode {
            1 => 1.0,
            2 => self.px_per_mm * 0.1 * -1.0,
            3 => self.px_per_mm * 0.01 * -1.0,
            4 => self.px_per_mm * 0.01 * 25.4 * -1.0,
            5 => self.px_per_mm * 0.001 * 25.4 * -1.0,
            6 => self.px_per_mm / 1440.0 * 25.4 * -1.0,
            7 => self.sf_x(),
            8 => {
                if coord.window_ext_set && coord.viewport_ext_set {
                    f64::from(coord.viewport_ext.1) / f64::from(coord.window_ext.1)
                } else if self.fix_broken_y {
                    -1.0
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    fn cal_x(&self, x: i64) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        if self.use_orgs() {
            (((x - i64::from(coord.window_org.0)) as f64 * self.sf_x())
                + f64::from(coord.viewport_org.0))
                * self.scaling
        } else {
            x as f64 * self.sf_x() * self.scaling
        }
    }

    fn cal_y(&self, y: i64) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        if self.use_orgs() {
            (((y - i64::from(coord.window_org.1)) as f64 * self.sf_y())
                + f64::from(coord.viewport_org.1))
                * self.scaling
        } else {
            y as f64 * self.sf_y() * self.scaling
        }
    }

    fn scale_dim_x(&self, w: i64) -> f64 {
        w as f64 * self.sf_x() * self.scaling
    }

    fn scale_dim_y(&self, h: i64) -> f64 {
        h as f64 * self.sf_y() * self.scaling
    }

    // --- walk ---

    fn walk(&mut self, document: &emf_core::Document, records: &[emf_core::RecordSpan]) {
        for (idx, span) in records.iter().enumerate() {
            // A comment hint applies to the next record only, and only when
            // the record's wire fingerprint matches the hint's.
            if let Some(pending) = self.pending_hint.take() {
                self.hint = None;
                let matches = document
                    .record_bytes(span)
                    .map(|bytes| {
                        pending.record_fingerprint == sha256_hex(bytes)
                    })
                    .unwrap_or(false);
                if matches {
                    self.hint = Some(pending);
                }
            }
            self.visit(document, idx, span);
            self.hint = None;
        }
    }

    fn finish(&mut self) {
        for _ in 0..self.open_groups {
            self.items.push(Item::GroupClose);
        }
        self.open_groups = 0;
    }

    fn visit(&mut self, document: &emf_core::Document, idx: usize, span: &emf_core::RecordSpan) {
        let record_type = span.record_type();
        let data = span.data();
        match data {
            emf_core::RecordData::Eof(_) => {}
            emf_core::RecordData::SetTextColor { color } => self.dc.text_color = color_of(color),
            emf_core::RecordData::SetBkColor { color } => self.dc.bk_color = color_of(color),
            emf_core::RecordData::SetBkMode { bk_mode } => self.dc.bk_mode = *bk_mode,
            emf_core::RecordData::SetMapMode { map_mode } => {
                self.dc.coordinate_state.borrow_mut().map_mode = *map_mode;
            }
            emf_core::RecordData::SetRop2 { .. } => {}
            emf_core::RecordData::SetPolyFillMode { poly_fill_mode } => {
                self.dc.poly_fill_mode = *poly_fill_mode;
            }
            emf_core::RecordData::SetTextAlign { text_align_mode } => {
                self.dc.text_align = *text_align_mode;
            }
            emf_core::RecordData::SetStretchBltMode { stretch_mode } => {
                self.dc.stretch_mode = *stretch_mode;
            }
            emf_core::RecordData::SaveDc => self.save_stack.push(&self.dc),
            emf_core::RecordData::RestoreDc { saved_dc } => {
                if let Some(snapshot) = self.save_stack.restore(*saved_dc) {
                    self.dc = snapshot.clone();
                }
            }
            emf_core::RecordData::SelectObject { ih_object } => self.select_object(*ih_object),
            emf_core::RecordData::DeleteObject { ih_object } => self.object_table.delete(*ih_object),
            emf_core::RecordData::CreatePen { ih_pen, pen_style, width, color } => {
                let pen = Pen { style: *pen_style, width: width.x, color: color_of(color) };
                self.object_table.store(*ih_pen, GdiObject::Pen(pen));
            }
            emf_core::RecordData::CreateBrushIndirect { ih_brush, brush_style, color, brush_hatch } => {
                let brush =
                    Brush { style: *brush_style, color: color_of(color), hatch: *brush_hatch, image_id: None };
                self.object_table.store(*ih_brush, GdiObject::Brush(brush));
            }
            emf_core::RecordData::CreateFontIndirectW(font_record) => {
                let font = font_from_record(&font_record.font);
                self.object_table.store(font_record.ih_object, GdiObject::Font(font));
            }
            emf_core::RecordData::SetWindowOrgEx { origin } => {
                self.dc.coordinate_state.borrow_mut().window_org = (origin.x, origin.y);
            }
            emf_core::RecordData::SetWindowExtEx { extent } => {
                let mut coord = self.dc.coordinate_state.borrow_mut();
                coord.window_ext = (extent.cx, extent.cy);
                coord.window_ext_set = true;
            }
            emf_core::RecordData::SetViewportOrgEx { origin } => {
                self.dc.coordinate_state.borrow_mut().viewport_org = (origin.x, origin.y);
            }
            emf_core::RecordData::SetViewportExtEx { extent } => {
                let mut coord = self.dc.coordinate_state.borrow_mut();
                coord.viewport_ext = (extent.cx, extent.cy);
                coord.viewport_ext_set = true;
            }
            emf_core::RecordData::MoveToEx { origin } => {
                self.current_x = f64::from(origin.x);
                self.current_y = f64::from(origin.y);
                if self.in_path {
                    let point = (self.cal_x(i64::from(origin.x)), self.cal_y(i64::from(origin.y)));
                    if let Some(geometry) = self.building.as_mut() {
                        geometry.push(Segment::MoveTo { x: point.0, y: point.1 });
                    }
                }
            }
            emf_core::RecordData::LineTo { origin } => {
                if self.in_path {
                    let point = (self.cal_x(i64::from(origin.x)), self.cal_y(i64::from(origin.y)));
                    if let Some(geometry) = self.building.as_mut() {
                        geometry.push(Segment::LineTo { x: point.0, y: point.1 });
                    }
                    self.current_x = f64::from(origin.x);
                    self.current_y = f64::from(origin.y);
                    return;
                }
                let mut geometry = self.start_standalone();
                geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(origin.x)),
                    y: self.cal_y(i64::from(origin.y)),
                });
                self.items.push(Item::Draw(Draw {
                    body: DrawBody::Path(PathDraw { geometry }),
                    paint: self.paint_now(),
                    arrangement: Arrangement::ContourOpen { leading_fill_space: true },
                }));
                self.current_x = f64::from(origin.x);
                self.current_y = f64::from(origin.y);
            }
            emf_core::RecordData::Rectangle { bounds } => {
                let (lt_x, lt_y, rb_x, rb_y) = self.cal_box(bounds);
                let dim_x = rb_x - lt_x;
                let mut dim_y = rb_y - lt_y;
                let mut lt_y = lt_y;
                if self.fix_broken_y && dim_y < 0.0 {
                    dim_y = -dim_y;
                    lt_y -= dim_y;
                }
                self.push_shape(ShapeKind::Rect { x: lt_x, y: lt_y, width: dim_x, height: dim_y });
            }
            emf_core::RecordData::Ellipse { bounds } => {
                let (lt_x, lt_y, rb_x, rb_y) = self.cal_box(bounds);
                let cx = (lt_x + rb_x) / 2.0;
                let cy = (lt_y + rb_y) / 2.0;
                let rx = (rb_x - lt_x).abs() / 2.0;
                let ry = (rb_y - lt_y).abs() / 2.0;
                self.push_shape(ShapeKind::Ellipse { cx, cy, rx, ry });
            }
            emf_core::RecordData::RoundRect { bounds, corner } => {
                let (lt_x, lt_y, rb_x, rb_y) = self.cal_box(bounds);
                let dim_x = rb_x - lt_x;
                let mut dim_y = rb_y - lt_y;
                let mut lt_y = lt_y;
                if self.fix_broken_y && dim_y < 0.0 {
                    dim_y = -dim_y;
                    lt_y -= dim_y;
                }
                // Both corner radii scale through the x dimension (reference
                // quirk).
                let rx = self.scale_dim_x(i64::from(corner.cx));
                let ry = self.scale_dim_x(i64::from(corner.cy));
                self.push_shape(ShapeKind::RoundedRect {
                    x: lt_x,
                    y: lt_y,
                    width: dim_x,
                    height: dim_y,
                    rx,
                    ry,
                });
            }
            emf_core::RecordData::Arc { bounds, start, end } => {
                self.emit_arc(*bounds, *start, *end, ArcMode::Arc);
            }
            emf_core::RecordData::ArcTo { bounds, start, end } => {
                self.emit_arc(*bounds, *start, *end, ArcMode::Arc);
            }
            emf_core::RecordData::Chord { bounds, start, end } => {
                self.emit_arc(*bounds, *start, *end, ArcMode::Chord);
            }
            emf_core::RecordData::Pie { bounds, start, end } => {
                self.emit_arc(*bounds, *start, *end, ArcMode::Pie);
            }
            emf_core::RecordData::Polygon { points, .. } => {
                self.emit_path_polygon_owned(points, true);
            }
            emf_core::RecordData::Polyline { points, .. } => {
                self.emit_path_polygon_owned(points, false);
            }
            emf_core::RecordData::Polygon16 { points, .. } => {
                self.emit_path_polygon16_owned(points, true);
            }
            emf_core::RecordData::Polyline16 { points, .. } => {
                self.emit_path_polygon16_owned(points, false);
            }
            emf_core::RecordData::PolylineTo { points, .. } => {
                self.emit_polyline_to_owned(points);
            }
            emf_core::RecordData::PolylineTo16 { points, .. } => {
                self.emit_polyline_to16_owned(points);
            }
            emf_core::RecordData::PolyBezier { points, .. } => {
                self.emit_poly_bezier_owned(points, true);
            }
            emf_core::RecordData::PolyBezier16 { points, .. } => {
                self.emit_poly_bezier16_owned(points, true);
            }
            emf_core::RecordData::PolyBezierTo { points, .. } => {
                self.emit_poly_bezier_owned(points, false);
            }
            emf_core::RecordData::PolyBezierTo16 { points, .. } => {
                self.emit_poly_bezier16_owned(points, false);
            }
            emf_core::RecordData::BeginPath => {
                if self.in_path {
                    self.pending_action = self.analysis.actions.get(&idx).copied().unwrap_or(PathAction::End);
                    return;
                }
                self.current_begin = Some(idx);
                self.apply_path_transform(true);
                self.building = Some(PathGeometry::new(CoordJoin::Tuple));
                self.in_path = true;
                self.pending_action = self.analysis.actions.get(&idx).copied().unwrap_or(PathAction::End);
            }
            emf_core::RecordData::EndPath => {
                self.in_path = false;
                self.finish_pending_path(self.pending_action);
                self.apply_path_transform(false);
                self.current_begin = None;
            }
            emf_core::RecordData::CloseFigure => {
                if self.in_path {
                    if let Some(geometry) = self.building.as_mut() {
                        geometry.push(Segment::Close);
                    }
                }
            }
            emf_core::RecordData::FillPath { .. } => {
                if !self.in_path {
                    return;
                }
                self.in_path = false;
                self.finish_pending_path(PathAction::Fill);
            }
            emf_core::RecordData::StrokePath { .. } => {
                if !self.in_path {
                    return;
                }
                self.in_path = false;
                self.finish_pending_path(PathAction::Stroke);
            }
            emf_core::RecordData::StrokeAndFillPath { .. } => {
                if !self.in_path {
                    return;
                }
                self.in_path = false;
                self.finish_pending_path(PathAction::StrokeFill);
            }
            emf_core::RecordData::AbortPath => {
                self.in_path = false;
                self.pending_action = PathAction::Abort;
            }
            emf_core::RecordData::SetWorldTransform { xform } => {
                let matrix = matrix_of(xform);
                self.dc.world_transform = [
                    matrix.m11 as f32,
                    matrix.m12 as f32,
                    matrix.m21 as f32,
                    matrix.m22 as f32,
                    matrix.dx as f32,
                    matrix.dy as f32,
                ];
                self.emit_transform_group(&matrix);
            }
            emf_core::RecordData::ModifyWorldTransform { xform, modify_mode } => {
                let xform_matrix = matrix_of(xform);
                let current = self.world_matrix();
                let new_matrix = match modify_mode {
                    1 => identity(),
                    2 => multiply(&xform_matrix, &current),
                    3 => multiply(&current, &xform_matrix),
                    _ => xform_matrix,
                };
                self.dc.world_transform = [
                    new_matrix.m11 as f32,
                    new_matrix.m12 as f32,
                    new_matrix.m21 as f32,
                    new_matrix.m22 as f32,
                    new_matrix.dx as f32,
                    new_matrix.dy as f32,
                ];
                self.emit_transform_group(&new_matrix);
            }
            emf_core::RecordData::SetMiterLimit { miter_limit } => {
                let value = f64::from(miter_limit.to_f32());
                // The subnormal/zero reinterpretation quirk: the float bit
                // pattern re-read as a little-endian u32.
                self.dc.miter_limit = if value.abs() < 1.175_494_350_822_287_5e-38 {
                    f64::from(miter_limit.to_bits())
                } else {
                    value
                };
            }
            emf_core::RecordData::Comment { data } => {
                self.pending_hint = hint::decode(data);
            }
            emf_core::RecordData::SetPixelV { .. } => {}
            emf_core::RecordData::ExtTextOutW(text) => {
                let content = if text.options & glyph::ETO_GLYPH_INDEX != 0 {
                    let font = &self.dc.font;
                    glyph::map_glyphs(&text.text, font.face_name.as_deref(), font.weight, font.italic, font.charset)
                        .unwrap_or_default()
                } else {
                    glyph::decode_utf16_with_question_mark(&text.text)
                };
                let x = self.cal_x(i64::from(text.reference.x));
                let y = self.cal_y(i64::from(text.reference.y));
                self.push_text_draw(x, y, content);
            }
            emf_core::RecordData::ExtTextOutA(text) => self.emit_ext_text_a(text),
            emf_core::RecordData::IntersectClipRect { clip } => {
                self.dc.clip_region = Some(crate::state::ClipRegion::Bounds(*clip));
                let region = self.dc.clip_region.clone();
                self.emit_clip_def(&region);
            }
            emf_core::RecordData::ExcludeClipRect { clip } => {
                self.dc.clip_region = Some(crate::state::ClipRegion::Bounds(*clip));
                let region = self.dc.clip_region.clone();
                self.emit_clip_def(&region);
            }
            emf_core::RecordData::SetBrushOrgEx { origin } => {
                self.dc.brush_origin = (origin.x, origin.y);
            }
            emf_core::RecordData::OffsetClipRgn { offset } => {
                let Some(region) = self.dc.clip_region.clone() else { return };
                let shifted = match region {
                    crate::state::ClipRegion::Bounds(rect) => {
                        Some(crate::state::ClipRegion::Bounds(emf_core::Rect {
                            left: rect.left + offset.x,
                            top: rect.top + offset.y,
                            right: rect.right + offset.x,
                            bottom: rect.bottom + offset.y,
                        }))
                    }
                    crate::state::ClipRegion::Path(geometry) => {
                        Some(crate::state::ClipRegion::Path(geometry))
                    }
                };
                self.dc.clip_region = shifted;
                let region = self.dc.clip_region.clone();
                self.emit_clip_def(&region);
            }
            emf_core::RecordData::SetIcmMode { mode } => self.dc.icm_mode = *mode,
            emf_core::RecordData::SetLayout { mode } => self.dc.layout = *mode,
            emf_core::RecordData::ScaleViewportExtEx { .. } => {}
            emf_core::RecordData::ScaleWindowExtEx { .. } => {}
            emf_core::RecordData::SetMapperFlags { flags } => self.dc.mapper_flags = *flags,
            emf_core::RecordData::SetArcDirection { arc_direction } => match arc_direction {
                2 => self.dc.arc_direction = 1,
                1 => self.dc.arc_direction = -1,
                _ => {}
            },
            emf_core::RecordData::RealizePalette => {}
            emf_core::RecordData::SelectPalette { .. } => {}
            emf_core::RecordData::CreatePalette { .. } => {}
            emf_core::RecordData::ResizePalette { .. } => {}
            emf_core::RecordData::SetPaletteEntries { .. } => {}
            emf_core::RecordData::PolyPolygon { poly_counts, points, .. } => {
                self.emit_poly_multi_owned(poly_counts, points_of(points), true);
            }
            emf_core::RecordData::PolyPolygon16 { poly_counts, points, .. } => {
                self.emit_poly_multi_owned(poly_counts, points16_of(points), true);
            }
            emf_core::RecordData::PolyPolyline { poly_counts, points, .. } => {
                self.emit_poly_multi_owned(poly_counts, points_of(points), false);
            }
            emf_core::RecordData::PolyPolyline16 { poly_counts, points, .. } => {
                self.emit_poly_multi_owned(poly_counts, points16_of(points), false);
            }
            emf_core::RecordData::PolyDraw { points, ab_types, .. } => {
                self.emit_poly_draw_owned(points_of(points), ab_types);
            }
            emf_core::RecordData::PolyDraw16 { points, ab_types, .. } => {
                self.emit_poly_draw_owned(points16_of(points), ab_types);
            }
            emf_core::RecordData::AngleArc { center, radius, start_angle, sweep_angle } => {
                self.emit_angle_arc(center, *radius, start_angle.to_f32(), sweep_angle.to_f32());
            }
            emf_core::RecordData::ExtCreatePen(pen) => {
                self.visit_ext_create_pen(document, span, pen);
            }
            emf_core::RecordData::Bitmap(bitmap) => {
                self.visit_bitmap(document, span, bitmap);
            }
            emf_core::RecordData::ExtSelectClipRgn { region_mode, .. } => {
                self.visit_ext_select_clip_rgn(document, span, *region_mode);
            }
            emf_core::RecordData::SelectClipPath { .. } => {
                let Some(geometry) = self.last_path_geometry.clone() else { return };
                if geometry.segments.is_empty() {
                    return;
                }
                self.dc.clip_region = Some(crate::state::ClipRegion::Path(geometry));
                let region = self.dc.clip_region.clone();
                self.emit_clip_def(&region);
            }
            emf_core::RecordData::GradientFill(_) => {}
            emf_core::RecordData::FillRgn { .. } => {}
            emf_core::RecordData::FrameRgn { .. } => {}
            emf_core::RecordData::InvertRgn { .. } => {}
            emf_core::RecordData::PaintRgn { .. } => {}
            emf_core::RecordData::PolyTextOutA(_) => {}
            emf_core::RecordData::PolyTextOutW(_) => {}
            emf_core::RecordData::SetTextJustification { .. } => {}
            emf_core::RecordData::SetColorAdjustment(_) => {}
            // SmallTextOut stays Raw in the model; parse its fixed prefix.
            emf_core::RecordData::Raw => {
                if record_type == 108 {
                    self.visit_small_text_out(document, span);
                }
            }
            // Records without reference handlers fall through silently.
            _ => {}
        }
    }

    // --- object table ---

    fn select_object(&mut self, ih: u32) {
        // Stock object?
        if ih & 0x8000_0000 != 0 {
            let index = ih & 0x7FFF_FFFF;
            self.apply_stock_object(index);
            return;
        }
        let Some(entry) = self.object_table.lookup(ih).cloned() else { return };
        match entry {
            GdiObject::Pen(pen) => self.dc.pen = pen,
            GdiObject::Brush(brush) => self.dc.brush = brush,
            GdiObject::Font(font) => self.dc.font = font,
        }
    }

    fn apply_stock_object(&mut self, index: u32) {
        // (kind, variant) per MS-EMF 2.1.7 stock indices.
        const WHITE_BRUSH: u32 = 0;
        const LTGRAY_BRUSH: u32 = 1;
        const GRAY_BRUSH: u32 = 2;
        const DKGRAY_BRUSH: u32 = 3;
        const BLACK_BRUSH: u32 = 4;
        const NULL_BRUSH: u32 = 5;
        const WHITE_PEN: u32 = 6;
        const BLACK_PEN: u32 = 7;
        const NULL_PEN: u32 = 8;
        match index {
            NULL_BRUSH => self.dc.brush = Brush::null(),
            WHITE_BRUSH => self.dc.brush = Brush::solid(Color::new(0xFF, 0xFF, 0xFF)),
            LTGRAY_BRUSH => self.dc.brush = Brush::solid(Color::new(0xC0, 0xC0, 0xC0)),
            GRAY_BRUSH => self.dc.brush = Brush::solid(Color::new(0x80, 0x80, 0x80)),
            DKGRAY_BRUSH => self.dc.brush = Brush::solid(Color::new(0x40, 0x40, 0x40)),
            BLACK_BRUSH => self.dc.brush = Brush::solid(Color::black()),
            NULL_PEN => self.dc.pen = Pen::null(),
            WHITE_PEN => self.dc.pen = Pen::solid(0, 1, Color::new(0xFF, 0xFF, 0xFF)),
            BLACK_PEN => self.dc.pen = Pen::solid(0, 1, Color::black()),
            _ => {}
        }
    }

    // --- paint ---

    fn paint_now(&self) -> PathPaint {
        // Every raw family carries the clip attachment (clipset_draw);
        // only the deferred path family omits it.
        PathPaint {
            fill: self.resolve_raw_fill(),
            stroke: Some(self.resolve_raw_stroke()),
            clip: self.dc.clip_id,
            stroke_from_fill: false,
        }
    }

    fn resolve_raw_fill(&self) -> Option<Fill> {
        let brush = &self.dc.brush;
        Some(Fill {
            style: match brush.style {
                1 => FillStyle::None,
                8 => FillStyle::PatternRef {
                    id: format!("img-{}-ref", brush.image_id.unwrap_or(0)),
                },
                _ => FillStyle::Solid { color: brush.color },
            },
        })
    }

    fn resolve_raw_stroke(&self) -> Stroke {
        let pen = &self.dc.pen;
        if pen.is_null() {
            return if self.dc.brush.is_null() {
                Stroke::NoneWithZeroWidth
            } else {
                Stroke::FromFill { color: self.dc.brush.color }
            };
        }
        let pen_type = pen.style & 0x000F_0000;
        let width_for_emit = if pen_type == 0x0001_0000 { i64::from(pen.width) } else { 1 };
        let width = self.resolve_width(width_for_emit);
        Stroke::Solid {
            color: pen.color,
            width,
            dash: self.resolve_dash_scaled(),
            cap: resolve_cap(pen.style),
            join: resolve_join(pen.style, self.scaling * self.dc.miter_limit),
        }
    }

    fn resolve_width(&self, width: i64) -> Width {
        let w = width.max(1);
        let tmp_w = self.scale_dim_x(w);
        if tmp_w / self.scaling < 1.0 {
            let pen = &self.dc.pen;
            let legacy = self.hint.as_ref().is_some_and(|hint| {
                hint.kind == "stroke"
                    && hint.values.first() == Some(&rgb_hex(pen.color))
                    && (1..=4).contains(&(pen.style & 0xFF))
            });
            if legacy {
                Width::OneLegacy
            } else {
                Width::OnePixel
            }
        } else {
            Width::Scaled(tmp_w)
        }
    }

    fn resolve_dash_scaled(&self) -> Option<Dash> {
        let unit = f64::from(self.dc.pen.width.max(1)) * self.scaling;
        let dash = unit * 5.0;
        let dot = unit;
        match self.dc.pen.style & 0xFF {
            1 => Some(Dash(vec![dash, dash])),
            2 => Some(Dash(vec![dot, dot])),
            3 => Some(Dash(vec![dash, dash, dot, dash])),
            4 => Some(Dash(vec![dash, dash, dot, dot, dot, dash])),
            _ => None,
        }
    }

    fn resolve_attrmap_fill(&mut self) -> Option<Fill> {
        let brush = &self.dc.brush;
        if brush.is_null() {
            return Some(Fill { style: FillStyle::None });
        }
        if brush.style == 2 {
            self.hatch_counter += 1;
            let id = format!("hatch-{}", self.hatch_counter);
            self.items.push(Item::DefineHatchPattern(HatchPatternDef {
                id: id.clone(),
                style: hatch_style_of(brush.hatch),
                foreground: brush.color,
                background: if self.dc.bk_mode == 2 { Some(self.dc.bk_color) } else { None },
            }));
            return Some(Fill { style: FillStyle::HatchRef { id } });
        }
        Some(Fill { style: FillStyle::Solid { color: brush.color } })
    }

    fn resolve_attrmap_stroke(&self) -> Stroke {
        let pen = &self.dc.pen;
        if pen.is_null() {
            return if self.dc.brush.is_null() {
                Stroke::NoneWithZeroWidth
            } else {
                Stroke::FromFill { color: self.dc.brush.color }
            };
        }
        let w = i64::from(pen.width.max(1));
        let dash_len = (w * 5) as f64;
        let dot_len = w as f64;
        let dash = match pen.style & 0xFF {
            1 => Some(Dash(vec![dash_len, dash_len])),
            2 => Some(Dash(vec![dot_len, dot_len])),
            3 => Some(Dash(vec![dash_len, dash_len, dot_len, dash_len])),
            4 => Some(Dash(vec![dash_len, dash_len, dot_len, dot_len, dot_len, dash_len])),
            _ => None,
        };
        Stroke::Solid {
            color: pen.color,
            width: Width::Count(w),
            dash,
            cap: resolve_cap(pen.style),
            join: resolve_join(pen.style, self.dc.miter_limit),
        }
    }

    // --- groups ---

    fn world_matrix(&self) -> MatrixF64 {
        let wt = &self.dc.world_transform;
        MatrixF64 {
            m11: f64::from(wt[0]),
            m12: f64::from(wt[1]),
            m21: f64::from(wt[2]),
            m22: f64::from(wt[3]),
            dx: f64::from(wt[4]),
            dy: f64::from(wt[5]),
        }
    }

    fn set_world_matrix(&mut self, matrix: &MatrixF64) {
        self.dc.world_transform = [
            matrix.m11 as f32,
            matrix.m12 as f32,
            matrix.m21 as f32,
            matrix.m22 as f32,
            matrix.dx as f32,
            matrix.dy as f32,
        ];
    }

    fn emit_transform_group(&mut self, matrix: &MatrixF64) {
        if self.in_path {
            return;
        }
        if self.open_groups > 0 {
            self.items.push(Item::GroupClose);
            self.open_groups -= 1;
        }
        let dx_scaled = matrix.dx * self.sf_x() * self.scaling;
        let dy_scaled = matrix.dy * self.sf_y() * self.scaling;
        self.items.push(Item::GroupOpen {
            transform: Matrix {
                m11: matrix.m11,
                m12: matrix.m12,
                m21: matrix.m21,
                m22: matrix.m22,
                dx: dx_scaled,
                dy: dy_scaled,
            },
        });
        self.open_groups += 1;
    }

    fn apply_path_transform(&mut self, before: bool) {
        let Some(begin) = self.current_begin else { return };
        let Some(entry) = self.analysis.transforms.get(&begin) else { return };
        let selected: Option<&PathTransform> =
            if before { entry.wt_before.as_ref() } else { entry.wt_after.as_ref() };
        let Some(pt) = selected else { return };
        let current = self.world_matrix();
        let new_matrix = match pt.mode {
            1 => identity(),
            2 => multiply(&matrix_of(&pt.matrix), &current),
            3 => multiply(&current, &matrix_of(&pt.matrix)),
            _ => matrix_of(&pt.matrix),
        };
        self.set_world_matrix(&new_matrix);
        self.emit_transform_group(&new_matrix);
    }

    // --- clip defs ---

    fn emit_clip_def(&mut self, region: &Option<crate::state::ClipRegion>) {
        let Some(region) = region else { return };
        let shape = match region {
            crate::state::ClipRegion::Bounds(rect) => {
                let mut geometry = PathGeometry::new(CoordJoin::Tuple);
                geometry.push(Segment::MoveTo {
                    x: self.cal_x(i64::from(rect.left)),
                    y: self.cal_y(i64::from(rect.top)),
                });
                geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(rect.right)),
                    y: self.cal_y(i64::from(rect.top)),
                });
                geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(rect.right)),
                    y: self.cal_y(i64::from(rect.bottom)),
                });
                geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(rect.left)),
                    y: self.cal_y(i64::from(rect.bottom)),
                });
                geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(rect.left)),
                    y: self.cal_y(i64::from(rect.top)),
                });
                geometry.push(Segment::Close);
                ClipShape::Bounds(geometry)
            }
            crate::state::ClipRegion::Path(geometry) => ClipShape::Path(geometry.clone()),
        };
        let id = self.bsd_rand.next();
        self.items.push(Item::DefineClip(ClipDef { id, shape }));
        self.dc.clip_id = Some(id);
    }

    // --- raster payload ---

    /// Resolves a decoded DIB into the VIM raster payload: RGBA surfaces
    /// encode PNG; BI_PNG payloads pass through; BI_JPEG payloads select the
    /// reference's mime and zero-padding quirk (hint-jpeg keeps the verbatim
    /// bytes under image/jpeg; otherwise the payload pads to a triple
    /// boundary under image/jpg). Returns (raster, width, height).
    fn href_for(&mut self, decoded: &raster::Decoded) -> Option<(Raster, i64, i64)> {
        match decoded {
            raster::Decoded::Rgba { width, height, rgba } => {
                let mut png = raster::encode_png_rgba8(*width as u32, *height as u32, rgba).ok()?;
                // The reference PngEncoder pads the PNG to a multiple of 3
                // with zero bytes (libemf2svg's fmem modulo-3 allocation), so
                // the base64 tail is `AA`/`A=` rather than `==`.
                let rem = png.len() % 3;
                if rem != 0 {
                    png.extend(std::iter::repeat_n(0u8, 3 - rem));
                }
                Some((
                    Raster { format: RasterFormat::Png, payload: png },
                    i64::from(*width),
                    i64::from(*height),
                ))
            }
            raster::Decoded::Png(bytes) => {
                Some((Raster { format: RasterFormat::Png, payload: bytes.clone() }, 0, 0))
            }
            raster::Decoded::Jpeg(bytes) => {
                if self.hint.as_ref().is_some_and(|hint| hint.kind == "jpeg") {
                    Some((
                        Raster { format: RasterFormat::Jpeg { zero_padded: false }, payload: bytes.clone() },
                        0,
                        0,
                    ))
                } else {
                    let mut payload = bytes.clone();
                    let rem = payload.len() % 3;
                    if rem != 0 {
                        payload.extend(std::iter::repeat_n(0u8, (3 - rem) % 3));
                    }
                    Some((
                        Raster { format: RasterFormat::Jpeg { zero_padded: true }, payload },
                        0,
                        0,
                    ))
                }
            }
        }
    }

    // --- geometry helpers ---

    fn cal_box(&self, bounds: &emf_core::Rect) -> (f64, f64, f64, f64) {
        (
            self.cal_x(i64::from(bounds.left)),
            self.cal_y(i64::from(bounds.top)),
            self.cal_x(i64::from(bounds.right)),
            self.cal_y(i64::from(bounds.bottom)),
        )
    }

    fn push_shape(&mut self, kind: ShapeKind) {
        let paint = self.paint_now();
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Shape(ShapeDraw { kind }),
            paint,
            arrangement: Arrangement::RegionFillFirst,
        }));
    }

    fn start_standalone(&mut self) -> PathGeometry {
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        geometry.push(Segment::MoveTo {
            x: self.cal_x(self.current_x.trunc() as i64),
            y: self.cal_y(self.current_y.trunc() as i64),
        });
        geometry
    }

    fn finish_pending_path(&mut self, mode: PathAction) {
        let Some(geometry) = self.building.take() else { return };
        self.last_path_geometry = Some(geometry.clone());
        let paint = PathPaint {
            fill: if matches!(mode, PathAction::Fill | PathAction::StrokeFill) {
                self.resolve_raw_fill()
            } else {
                None
            },
            stroke: if matches!(mode, PathAction::Stroke | PathAction::StrokeFill) {
                Some(self.resolve_raw_stroke())
            } else {
                None
            },
            clip: None,
            stroke_from_fill: false,
        };
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint,
            arrangement: Arrangement::DeferredPath,
        }));
    }

    // --- poly families ---

    fn emit_path_polygon_owned(&mut self, points: &[emf_core::Point], closed: bool) {
        let owned: Vec<(i32, i32)> = points.iter().map(|p| (p.x, p.y)).collect();
        self.emit_path_polygon(&owned, closed);
    }

    fn emit_path_polygon16_owned(&mut self, points: &[emf_core::Point16], closed: bool) {
        let owned: Vec<(i32, i32)> = points.iter().map(|p| (i32::from(p.x), i32::from(p.y))).collect();
        self.emit_path_polygon(&owned, closed);
    }

    fn emit_path_polygon(&mut self, points: &[(i32, i32)], closed: bool) {
        if points.is_empty() {
            return;
        }
        if self.in_path {
            let calibrated: Vec<(f64, f64)> = points
                .iter()
                .map(|(x, y)| (self.cal_x(i64::from(*x)), self.cal_y(i64::from(*y))))
                .collect();
            if let Some(geometry) = self.building.as_mut() {
                geometry.push(Segment::MoveTo { x: calibrated[0].0, y: calibrated[0].1 });
                for point in &calibrated[1..] {
                    geometry.push(Segment::LineTo { x: point.0, y: point.1 });
                }
            }
            let (xl, yl) = points[points.len() - 1];
            self.current_x = f64::from(xl);
            self.current_y = f64::from(yl);
            return;
        }
        // The standalone polygon family starts at the record's FIRST point
        // (no current-position prefix — that is the line-family's form).
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        let (x0, y0) = points[0];
        geometry.push(Segment::MoveTo {
            x: self.cal_x(i64::from(x0)),
            y: self.cal_y(i64::from(y0)),
        });
        for (x, y) in &points[1..] {
            geometry.push(Segment::LineTo {
                x: self.cal_x(i64::from(*x)),
                y: self.cal_y(i64::from(*y)),
            });
        }
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: self.paint_now(),
            arrangement: if closed {
                Arrangement::RegionStrokeFirst
            } else {
                Arrangement::ContourOpen { leading_fill_space: false }
            },
        }));
        let (xl, yl) = points[points.len() - 1];
        self.current_x = f64::from(xl);
        self.current_y = f64::from(yl);
    }

    fn emit_polyline_to_owned(&mut self, points: &[emf_core::Point]) {
        let owned: Vec<(i32, i32)> = points.iter().map(|p| (p.x, p.y)).collect();
        self.emit_polyline_to(&owned);
    }

    fn emit_polyline_to16_owned(&mut self, points: &[emf_core::Point16]) {
        let owned: Vec<(i32, i32)> = points.iter().map(|p| (i32::from(p.x), i32::from(p.y))).collect();
        self.emit_polyline_to(&owned);
    }

    fn emit_polyline_to(&mut self, points: &[(i32, i32)]) {
        if points.is_empty() {
            return;
        }
        if self.in_path {
            let calibrated: Vec<(f64, f64)> = points
                .iter()
                .map(|(x, y)| (self.cal_x(i64::from(*x)), self.cal_y(i64::from(*y))))
                .collect();
            if let Some(geometry) = self.building.as_mut() {
                for point in &calibrated {
                    geometry.push(Segment::LineTo { x: point.0, y: point.1 });
                }
            }
            let (xl, yl) = points[points.len() - 1];
            self.current_x = f64::from(xl);
            self.current_y = f64::from(yl);
            return;
        }
        let mut geometry = self.start_standalone();
        for (x, y) in points {
            geometry.push(Segment::LineTo {
                x: self.cal_x(i64::from(*x)),
                y: self.cal_y(i64::from(*y)),
            });
        }
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: self.paint_now(),
            arrangement: Arrangement::ContourOpen { leading_fill_space: true },
        }));
        let (xl, yl) = points[points.len() - 1];
        self.current_x = f64::from(xl);
        self.current_y = f64::from(yl);
    }

    fn emit_poly_bezier_owned(&mut self, points: &[emf_core::Point], starting_point: bool) {
        let owned: Vec<(i32, i32)> = points.iter().map(|p| (p.x, p.y)).collect();
        self.emit_poly_bezier(&owned, starting_point);
    }

    fn emit_poly_bezier16_owned(&mut self, points: &[emf_core::Point16], starting_point: bool) {
        let owned: Vec<(i32, i32)> =
            points.iter().map(|p| (i32::from(p.x), i32::from(p.y))).collect();
        self.emit_poly_bezier(&owned, starting_point);
    }

    fn emit_poly_bezier(&mut self, points: &[(i32, i32)], starting_point: bool) {
        if points.is_empty() {
            return;
        }
        // Precompute the calibrated curve points (avoids borrowing the
        // builder and the mapper against each other).
        let cal_points: Vec<(f64, f64)> = points
            .iter()
            .map(|(x, y)| (self.cal_x(i64::from(*x)), self.cal_y(i64::from(*y))))
            .collect();
        if self.in_path {
            if starting_point {
                if let Some(geometry) = self.building.as_mut() {
                    let (x, y) = cal_points[0];
                    geometry.push(Segment::MoveTo { x, y });
                }
            }
            let ctrls: &[(f64, f64)] = if starting_point { &cal_points[1..] } else { &cal_points };
            if let Some(geometry) = self.building.as_mut() {
                push_bezier_triplets(geometry, ctrls);
            }
            let (xl, yl) = points[points.len() - 1];
            self.current_x = f64::from(xl);
            self.current_y = f64::from(yl);
            return;
        }
        let mut geometry = self.start_standalone();
        if starting_point {
            let (x, y) = cal_points[0];
            geometry.push(Segment::MoveTo { x, y });
        }
        let ctrls: &[(f64, f64)] = if starting_point { &cal_points[1..] } else { &cal_points };
        push_bezier_triplets(&mut geometry, ctrls);
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: self.paint_now(),
            arrangement: Arrangement::ContourOpen { leading_fill_space: true },
        }));
        let (xl, yl) = points[points.len() - 1];
        self.current_x = f64::from(xl);
        self.current_y = f64::from(yl);
    }

    fn emit_poly_multi_owned(
        &mut self,
        poly_counts: &[u32],
        points: Vec<(i32, i32)>,
        closed: bool,
    ) {
        self.emit_poly_multi(poly_counts, &points, closed);
    }

    fn emit_poly_multi(&mut self, counts: &[u32], all_points: &[(i32, i32)], closed: bool) {
        if counts.is_empty() || all_points.is_empty() {
            return;
        }
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        let mut offset = 0usize;
        let mut last: Option<(i32, i32)> = None;
        let mut any = false;
        for count in counts {
            let count = *count as usize;
            if offset + count > all_points.len() {
                offset += count;
                continue;
            }
            let sub_points = &all_points[offset..offset + count];
            offset += count;
            if sub_points.len() != count {
                continue;
            }
            any = true;
            let calibrated: Vec<(f64, f64)> = sub_points
                .iter()
                .map(|(x, y)| (self.cal_x(i64::from(*x)), self.cal_y(i64::from(*y))))
                .collect();
            let (x0, y0) = calibrated[0];
            geometry.push(Segment::MoveTo { x: x0, y: y0 });
            for point in &calibrated[1..] {
                geometry.push(Segment::LineTo { x: point.0, y: point.1 });
            }
            if closed {
                geometry.push(Segment::Close);
            }
            last = Some(*sub_points.last().expect("non-empty sub"));
        }
        if !any || geometry.segments.is_empty() {
            return;
        }
        if self.in_path {
            if let Some(building) = self.building.as_mut() {
                building.segments.extend(geometry.segments);
            }
        } else {
            // The multi-region families paint through the RAW fill/stroke
            // resolution (fmt'd widths), not the attribute-map family.
            let paint = self.paint_now();
            self.items.push(Item::Draw(Draw {
                body: DrawBody::Path(PathDraw { geometry }),
                paint,
                arrangement: if closed {
                    Arrangement::RegionFillFirst
                } else {
                    Arrangement::ContourOpen { leading_fill_space: false }
                },
            }));
        }
        if let Some((xl, yl)) = last {
            self.current_x = f64::from(xl);
            self.current_y = f64::from(yl);
        }
    }

    fn emit_poly_draw_owned(&mut self, points: Vec<(i32, i32)>, types: &[u8]) {
        self.emit_poly_draw(&points, types);
    }

    fn emit_poly_draw(&mut self, points: &[(i32, i32)], types: &[u8]) {
        if points.is_empty() || types.is_empty() {
            return;
        }
        let mut geometry = PathGeometry::new(CoordJoin::Sequence);
        let mut bezier_buf: Vec<(f64, f64)> = Vec::new();
        let mut i = 0usize;
        for t in types {
            let t = *t;
            let Some(point) = points.get(i) else { break };
            i += 1;
            match t & 0x07 {
                1 => geometry.push(Segment::MoveTo {
                    x: self.cal_x(i64::from(point.0)),
                    y: self.cal_y(i64::from(point.1)),
                }),
                2 => geometry.push(Segment::LineTo {
                    x: self.cal_x(i64::from(point.0)),
                    y: self.cal_y(i64::from(point.1)),
                }),
                4 => {
                    bezier_buf
                        .push((self.cal_x(i64::from(point.0)), self.cal_y(i64::from(point.1))));
                    if bezier_buf.len() == 3 {
                        let (x1, y1) = bezier_buf[0];
                        let (x2, y2) = bezier_buf[1];
                        let (x3, y3) = bezier_buf[2];
                        geometry.push(Segment::CubicTo { x1, y1, x2, y2, x: x3, y: y3 });
                        bezier_buf.clear();
                    }
                }
                _ => {}
            }
            if t & 8 != 0 {
                geometry.push(Segment::Close);
            }
        }
        // Attribute-map family: fill via fill_attrs, stroke via stroke_attrs.
        let fill = self.resolve_attrmap_fill();
        let stroke = self.resolve_attrmap_stroke();
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: PathPaint {
                fill,
                stroke: Some(stroke),
                clip: self.dc.clip_id,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::AttrMap,
        }));
    }

    // --- arcs ---

    fn emit_angle_arc(&mut self, center: &emf_core::Point, radius: u32, start_angle: f32, sweep_angle: f32) {
        let cx_raw = i64::from(center.x);
        let cy_raw = i64::from(center.y);
        let r_raw = i64::from(radius);
        let arcdir = self.dc.arc_direction;
        let sweep_flag = i32::from(arcdir > 0);
        let large_arc_flag = i32::from(arcdir > 0);

        let start_rad = f64::from(start_angle) * std::f64::consts::PI / 180.0;
        let end_rad = (f64::from(start_angle) + f64::from(sweep_angle)) * std::f64::consts::PI / 180.0;
        let sx_raw = r_raw as f64 * f64::cos(start_rad) + cx_raw as f64;
        let sy_raw = r_raw as f64 * f64::sin(start_rad) + cy_raw as f64;
        let ex_raw = r_raw as f64 * f64::cos(end_rad) + cx_raw as f64;
        let ey_raw = r_raw as f64 * f64::sin(end_rad) + cy_raw as f64;

        let mut geometry = self.start_standalone();
        geometry.push(Segment::MoveTo { x: self.cal_x_f(sx_raw), y: self.cal_y_f(sy_raw) });
        geometry.push(Segment::ArcTo {
            rx: self.cal_x_f(r_raw as f64),
            ry: self.cal_y_f(r_raw as f64),
            rotation: 0.0,
            large: large_arc_flag == 1,
            sweep: sweep_flag == 1,
            x: self.cal_x_f(ex_raw),
            y: self.cal_y_f(ey_raw),
        });
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: self.paint_now(),
            arrangement: Arrangement::ContourOpen { leading_fill_space: true },
        }));
    }

    fn emit_arc(&mut self, bounds: emf_core::Rect, start: emf_core::Point, end: emf_core::Point, mode: ArcMode) {
        let radii_x = c_int_div(i64::from(bounds.right - bounds.left), 2);
        let radii_y = c_int_div(i64::from(bounds.bottom - bounds.top), 2);
        if radii_x == 0.0 || radii_y == 0.0 {
            return;
        }

        // Raw (window-space) intersections; the reference leaves the current
        // position in window space.
        let start_pt = self.int_el_rad(&start, &bounds, radii_x, radii_y);
        let end_pt = self.int_el_rad(&end, &bounds, radii_x, radii_y);

        let arcdir = self.dc.arc_direction;
        let sweep_flag = i32::from(arcdir > 0);
        let large_arc_flag = i32::from(arcdir > 0);

        let mut geometry = self.start_standalone();
        geometry.push(Segment::MoveTo { x: self.cal_x_f(start_pt.0), y: self.cal_y_f(start_pt.1) });
        geometry.push(Segment::ArcTo {
            rx: self.cal_x_f(radii_x),
            ry: self.cal_y_f(radii_y),
            rotation: 0.0,
            large: large_arc_flag == 1,
            sweep: sweep_flag == 1,
            x: self.cal_x_f(end_pt.0),
            y: self.cal_y_f(end_pt.1),
        });

        match mode {
            ArcMode::Pie => {
                // Ruby Integer division floors here (plain `/ 2`).
                let center_x = floor_div(i64::from(bounds.right) + i64::from(bounds.left), 2) as f64;
                let center_y = floor_div(i64::from(bounds.bottom) + i64::from(bounds.top), 2) as f64;
                geometry.push(Segment::LineTo {
                    x: self.cal_x_f(center_x),
                    y: self.cal_y_f(center_y),
                });
                geometry.push(Segment::Close);
                self.items.push(Item::Draw(Draw {
                    body: DrawBody::Path(PathDraw { geometry }),
                    paint: self.paint_now(),
                    arrangement: Arrangement::FormClosed,
                }));
                self.current_x = center_x;
                self.current_y = center_y;
            }
            ArcMode::Chord => {
                geometry.push(Segment::Close);
                self.items.push(Item::Draw(Draw {
                    body: DrawBody::Path(PathDraw { geometry }),
                    paint: self.paint_now(),
                    arrangement: Arrangement::FormClosed,
                }));
                self.current_x = end_pt.0;
                self.current_y = end_pt.1;
            }
            ArcMode::Arc => {
                self.items.push(Item::Draw(Draw {
                    body: DrawBody::Path(PathDraw { geometry }),
                    paint: self.paint_now(),
                    arrangement: Arrangement::ContourOpen { leading_fill_space: true },
                }));
                self.current_x = end_pt.0;
                self.current_y = end_pt.1;
            }
        }
    }

    /// The ellipse-boundary intersection (the reference int_el_rad) in RAW
    /// window space, with C truncating integer division for centers/radii.
    fn int_el_rad(
        &self,
        point: &emf_core::Point,
        rect: &emf_core::Rect,
        radii_x: f64,
        radii_y: f64,
    ) -> (f64, f64) {
        let center_x = c_int_div(i64::from(rect.right) + i64::from(rect.left), 2);
        let center_y = c_int_div(i64::from(rect.bottom) + i64::from(rect.top), 2);
        if radii_x == 0.0 || radii_y == 0.0 {
            return (center_x, center_y);
        }
        let pt_no_x = f64::from(point.x) - center_x;
        let pt_no_y = f64::from(point.y) - center_y;
        if pt_no_x == 0.0 {
            let sign = if pt_no_y < 0.0 { -1.0 } else { 1.0 };
            return (center_x, sign * radii_y + center_y);
        }
        if pt_no_y == 0.0 {
            let sign = if pt_no_x < 0.0 { -1.0 } else { 1.0 };
            return (sign * radii_x + center_x, center_y);
        }
        let slope = pt_no_y / pt_no_x;
        let isect_x = (if pt_no_x < 0.0 { -1.0 } else { 1.0 })
            * f64::sqrt(1.0 / ((1.0 / radii_x).powi(2) + (slope / radii_y).powi(2)))
            + center_x;
        let isect_y = (if pt_no_y < 0.0 { -1.0 } else { 1.0 })
            * f64::sqrt(1.0 / ((1.0 / (slope * radii_x)).powi(2) + (1.0 / radii_y).powi(2)))
            + center_y;
        (isect_x, isect_y)
    }

    fn cal_x_f(&self, x: f64) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        if self.use_orgs() {
            ((x - f64::from(coord.window_org.0)) * self.sf_x() + f64::from(coord.viewport_org.0))
                * self.scaling
        } else {
            x * self.sf_x() * self.scaling
        }
    }

    fn cal_y_f(&self, y: f64) -> f64 {
        let coord = self.dc.coordinate_state.borrow();
        if self.use_orgs() {
            ((y - f64::from(coord.window_org.1)) * self.sf_y() + f64::from(coord.viewport_org.1))
                * self.scaling
        } else {
            y * self.sf_y() * self.scaling
        }
    }

    // --- text ---

    fn push_text_draw(&mut self, x: f64, y: f64, content: String) {
        let font = self.dc.font.clone();
        let font_height = self.scale_dim_x(i64::from(font.height)).abs();
        let rotation = if font.escapement != 0 {
            let orientation = if self.sf_y() > 0.0 { -1.0 } else { 1.0 };
            Some((orientation * f64::from(font.escapement) / 10.0).trunc() as i32)
        } else {
            None
        };
        let align = self.dc.text_align;
        let anchor = if align & 0x06 == 0x06 || align & 0x04 == 0x04 {
            TextAnchor::Middle
        } else if align & 0x02 == 0x02 {
            TextAnchor::End
        } else {
            TextAnchor::Start
        };
        let baseline = if align & 0x18 == 0x18 || align & 0x18 == 0x08 {
            BaselineMode::AtY
        } else {
            BaselineMode::Raised
        };
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Text(TextDraw {
                x,
                y,
                font_height,
                face: font.face_name.clone(),
                color: self.dc.text_color,
                rotation,
                italic: font.italic,
                underline: font.underline,
                strikeout: font.strikeout,
                weight: i64::from(font.weight),
                anchor,
                baseline,
                content,
            }),
            paint: PathPaint {
                fill: None,
                stroke: None,
                clip: self.dc.clip_id,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::RegionFillFirst,
        }));
    }

    fn emit_ext_text_a(&mut self, text: &emf_core::ExtTextOut<u8>) {
        let content = if text.options & glyph::ETO_GLYPH_INDEX != 0 {
            // An A record with glyph indices stores the ids as LE u16 pairs.
            let units: Vec<u16> =
                text.text.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let font = &self.dc.font;
            glyph::map_glyphs(&units, font.face_name.as_deref(), font.weight, font.italic, font.charset)
                .unwrap_or_default()
        } else {
            // Bytes above DEL collapse to spaces (reference cleaning).
            text.text.iter().map(|b| if *b > 0x7F { 0x20 } else { *b }).map(|b| b as char).collect()
        };
        let x = self.cal_x(i64::from(text.reference.x));
        let y = self.cal_y(i64::from(text.reference.y));
        self.push_text_draw(x, y, content);
    }

    fn visit_small_text_out(&mut self, document: &emf_core::Document, span: &emf_core::RecordSpan) {
        let Some(bytes) = document.record_bytes(span) else { return };
        if bytes.len() < 52 {
            return;
        }
        let read_u32 = |at: usize| -> u32 {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let read_i32 = |at: usize| -> i32 { read_u32(at) as i32 };
        let x_dest = read_i32(36);
        let y_dest = read_i32(40);
        let c_chars = read_u32(44) as usize;
        let fu = read_u32(48);
        let body = &bytes[52..];
        let text_offset = if fu & 0x100 == 0 { 16 } else { 0 };
        let small_chars = fu & 0x200 != 0;
        let bytes_per_char = if small_chars { 1 } else { 2 };
        let end = (text_offset + c_chars * bytes_per_char).min(body.len());
        let text_bytes = if text_offset <= body.len() { &body[text_offset..end] } else { &[] };
        let content = if small_chars {
            cp1252_to_utf8(text_bytes)
        } else {
            glyph::decode_utf16_with_question_mark(
                &text_bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<u16>>(),
            )
        };
        let x = self.cal_x(i64::from(x_dest));
        let y = self.cal_y(i64::from(y_dest));
        self.push_text_draw(x, y, content);
    }

    // --- regions ---

    fn visit_ext_select_clip_rgn(
        &mut self,
        document: &emf_core::Document,
        span: &emf_core::RecordSpan,
        region_mode: u32,
    ) {
        // RGN_COPY (5) with no region data resets the clip.
        if region_mode == 5 || region_mode == 0 {
            self.dc.clip_region = None;
            self.dc.clip_id = None;
            return;
        }
        // RGNDATAHEADER.rclBounds at offset 16 of the RGNDATA; the record
        // carries cbRgnData at offset 16 and the data at 20.
        let bounds = document.record_bytes(span).and_then(|bytes| {
            if bytes.len() < 52 {
                return None;
            }
            let read_i32 = |at: usize| -> i32 {
                i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
            };
            let base = 20;
            Some(emf_core::Rect {
                left: read_i32(base + 16),
                top: read_i32(base + 20),
                right: read_i32(base + 24),
                bottom: read_i32(base + 28),
            })
        });
        self.dc.clip_region = Some(match bounds {
            Some(rect) => crate::state::ClipRegion::Bounds(rect),
            None => crate::state::ClipRegion::Path(PathGeometry::new(CoordJoin::Tuple)),
        });
        let region = self.dc.clip_region.clone();
        self.emit_clip_def(&region);
    }

    // --- pens ---

    fn visit_ext_create_pen(
        &mut self,
        document: &emf_core::Document,
        span: &emf_core::RecordSpan,
        pen: &emf_core::ExtCreatePen,
    ) {
        // The EXTLOGPEN body: pen_style at body offset 16, width at 20, the
        // colour at 28..31 — where body starts after EMR + ihPen (12).
        let Some(bytes) = document.record_bytes(span) else { return };
        let body = bytes.get(12..).unwrap_or(&[]);
        if body.len() >= 32 {
            let pen_style = u32::from_le_bytes([body[16], body[17], body[18], body[19]]);
            let width = u32::from_le_bytes([body[20], body[21], body[22], body[23]]);
            let color = Color::new(body[28], body[29], body[30]);
            self.object_table.store(
                pen.ih_pen,
                GdiObject::Pen(Pen { style: pen_style, width: width as i32, color }),
            );
        } else {
            self.object_table.store(pen.ih_pen, GdiObject::Pen(Pen::default()));
        }
    }

    // --- bitmaps ---

    fn visit_bitmap(
        &mut self,
        document: &emf_core::Document,
        span: &emf_core::RecordSpan,
        bitmap: &emf_core::BitmapRecord,
    ) {
        match bitmap {
            emf_core::BitmapRecord::BitBlt { transfer, raster_operation } => {
                let bits_present = transfer.buffers.source_bits.as_ref().is_some_and(|b| !b.data.is_empty());
                if !bits_present {
                    self.emit_bitblt_solid_fill(transfer, *raster_operation);
                    return;
                }
                self.emit_image_from_buffers(document, span, transfer, true);
            }
            emf_core::BitmapRecord::StretchBlt { transfer, source_extent, .. } => {
                let bits_present = transfer
                    .buffers
                    .source_bits
                    .as_ref()
                    .is_some_and(|b| !b.data.is_empty());
                if bits_present {
                    self.emit_image_from_buffers(document, span, transfer, true);
                } else {
                    let b = transfer.bounds;
                    self.emit_image_from_body_buffers(
                        &transfer.buffers,
                        i64::from(b.left),
                        i64::from(b.top),
                        i64::from(b.right - b.left),
                        i64::from(b.bottom - b.top),
                    );
                    let _ = source_extent;
                }
            }
            emf_core::BitmapRecord::AlphaBlend { transfer, .. }
            | emf_core::BitmapRecord::TransparentBlt { transfer, .. } => {
                let b = transfer.bounds;
                self.emit_image_from_body_buffers(
                    &transfer.buffers,
                    i64::from(b.left),
                    i64::from(b.top),
                    i64::from(b.right - b.left),
                    i64::from(b.bottom - b.top),
                );
            }
            emf_core::BitmapRecord::StretchDibits { transfer, destination_extent, .. } => {
                self.emit_stretch_dibits(transfer, destination_extent);
            }
            emf_core::BitmapRecord::CreateMonoBrush { ih_brush, .. }
            | emf_core::BitmapRecord::CreateDibPatternBrushPt { ih_brush, .. } => {
                self.register_pattern_brush(document, span, *ih_brush);
            }
            _ => {}
        }
    }

    fn emit_stretch_dibits(
        &mut self,
        transfer: &emf_core::DibTransfer,
        destination_extent: &emf_core::Point,
    ) {
        let Some(bmi) = transfer.buffers.source_info.as_ref() else { return };
        if bmi.data.is_empty() {
            return;
        }
        let Some(bits) = transfer.buffers.source_bits.as_ref() else { return };
        let mut dib = bmi.data.clone();
        dib.extend_from_slice(&bits.data);
        let cb_bmi = bmi.data.len();
        let Some(decoded) = raster::decode_dib(&dib, Some(cb_bmi), None) else { return };
        let Some((raster_payload, _, _)) = self.href_for(&decoded) else { return };
        let x = i64::from(transfer.destination.x);
        let y = i64::from(transfer.destination.y);
        let width = i64::from(destination_extent.x);
        let height = i64::from(destination_extent.y);
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Image(ImageDraw {
                raster: raster_payload,
                arrangement: ImageArrangement::Calibrated {
                    width: self.cal_x(width),
                    height: self.cal_y(height),
                    x: self.cal_x(x),
                    y: self.cal_y(y),
                },
            }),
            paint: PathPaint {
                fill: None,
                stroke: None,
                clip: self.dc.clip_id,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::RegionFillFirst,
        }));
    }

    /// BitBlt/StretchBlt image emission through the calibrated placement
    /// (the reference maps both position AND size through point_cal).
    fn emit_image_from_buffers(
        &mut self,
        _document: &emf_core::Document,
        _span: &emf_core::RecordSpan,
        transfer: &emf_core::BitmapTransfer,
        _position_via_point_cal: bool,
    ) {
        let Some(bmi) = transfer.buffers.source_info.as_ref() else { return };
        if bmi.data.is_empty() {
            return;
        }
        let Some(bits) = transfer.buffers.source_bits.as_ref() else { return };
        let mut dib = bmi.data.clone();
        dib.extend_from_slice(&bits.data);
        let cb_bmi = bmi.data.len();
        let Some(decoded) = raster::decode_dib(&dib, Some(cb_bmi), None) else { return };
        let Some((raster_payload, _, _)) = self.href_for(&decoded) else { return };
        let x = i64::from(transfer.destination.x);
        let y = i64::from(transfer.destination.y);
        let width = i64::from(transfer.destination_extent.x);
        let height = i64::from(transfer.destination_extent.y);
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Image(ImageDraw {
                raster: raster_payload,
                arrangement: ImageArrangement::Calibrated {
                    width: self.cal_x(width),
                    height: self.cal_y(height),
                    x: self.cal_x(x),
                    y: self.cal_y(y),
                },
            }),
            paint: PathPaint {
                fill: None,
                stroke: None,
                clip: self.dc.clip_id,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::RegionFillFirst,
        }));
    }

    /// AlphaBlend/TransparentBlt/StretchBlt fallback: the bitmap is decoded
    /// from the raw body; placement uses the region bounds (x/width/height
    /// integral, y mapped — the reference's mixed forms).
    fn emit_image_from_body_buffers(
        &mut self,
        buffers: &emf_core::BitmapBuffers,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
    ) {
        let mut body = Vec::new();
        for kind in [
            emf_core::BitmapBufferKind::SourceInfo,
            emf_core::BitmapBufferKind::SourceBits,
            emf_core::BitmapBufferKind::MaskInfo,
            emf_core::BitmapBufferKind::MaskBits,
        ] {
            if let Some(buffer) = buffers.get(kind) {
                body.extend_from_slice(&buffer.padding);
                body.extend_from_slice(&buffer.data);
            }
        }
        if body.len() < 40 {
            return;
        }
        let Some(decoded) = raster::decode_dib(&body, None, None) else { return };
        let Some((raster_payload, _, _)) = self.href_for(&decoded) else { return };
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Image(ImageDraw {
                raster: raster_payload,
                arrangement: ImageArrangement::BodyBounds {
                    x,
                    y: self.cal_y(y),
                    width,
                    height,
                },
            }),
            paint: PathPaint {
                fill: None,
                stroke: None,
                clip: self.dc.clip_id,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::RegionFillFirst,
        }));
    }

    /// BitBlt with no source bitmap: a brush-filled rectangle path.
    fn emit_bitblt_solid_fill(&mut self, transfer: &emf_core::BitmapTransfer, rop: u32) {
        if rop == 0x00AA_0029 {
            return;
        }
        if self.dc.brush.style != 0 {
            return;
        }

        // The provenance-rect hint path (round-trip provenance).
        if self.hint.as_ref().is_some_and(|hint| hint.kind == "rect") && rop == 0x00F0_0021 {
            let hint = self.hint.clone().expect("hint checked");
            let pos_x = self.cal_x(i64::from(transfer.destination.x));
            let pos_y = self.cal_y(i64::from(transfer.destination.y));
            let size_x = self.scale_dim_x(i64::from(transfer.destination_extent.x));
            let size_y = self.scale_dim_y(i64::from(transfer.destination_extent.y));
            let geometry_match = if hint.values.len() == 5 {
                let parsed: Vec<f64> = hint.values[..4]
                    .iter()
                    .filter_map(|v| v.parse::<f64>().ok())
                    .collect();
                parsed.len() == 4 && parsed == vec![pos_x, pos_y, size_x, size_y]
            } else {
                false
            };
            if geometry_match && hint.values.last().map(|v| v == &rgb_hex(self.dc.brush.color)).unwrap_or(false) {
                let values = hint.values.clone();
                self.items.push(Item::Draw(Draw {
                    body: DrawBody::BareRect(eikogram::BareRect {
                        x: values[0].clone(),
                        y: values[1].clone(),
                        width: values[2].clone(),
                        height: values[3].clone(),
                        fill: values[4].clone(),
                    }),
                    paint: PathPaint {
                        fill: None,
                        stroke: None,
                        clip: None,
                        stroke_from_fill: false,
                    },
                    arrangement: Arrangement::BareShape,
                }));
                return;
            }
        }

        let color = self.dc.brush.color;
        let size_x = self.scale_dim_x(i64::from(transfer.destination_extent.x));
        let size_y = self.scale_dim_y(i64::from(transfer.destination_extent.y));
        let pos_x = self.cal_x(i64::from(transfer.destination.x));
        let pos_y = self.cal_y(i64::from(transfer.destination.y));
        // The reference's d closes with a bare `Z` (no trailing space), so
        // the close rides the arrangement, not a Close segment.
        let mut geometry = PathGeometry::new(CoordJoin::Tuple);
        geometry.push(Segment::MoveTo { x: pos_x, y: pos_y });
        geometry.push(Segment::LineTo { x: pos_x + size_x, y: pos_y });
        geometry.push(Segment::LineTo { x: pos_x + size_x, y: pos_y + size_y });
        geometry.push(Segment::LineTo { x: pos_x, y: pos_y + size_y });
        self.items.push(Item::Draw(Draw {
            body: DrawBody::Path(PathDraw { geometry }),
            paint: PathPaint {
                fill: Some(Fill { style: FillStyle::Solid { color } }),
                stroke: None,
                clip: None,
                stroke_from_fill: false,
            },
            arrangement: Arrangement::SolidRectFill,
        }));
    }

    /// CreateMonoBrush / CreateDibPatternBrushPt: extract the embedded DIB,
    /// emit the raster pattern definition, and store a monopattern brush.
    fn register_pattern_brush(
        &mut self,
        document: &emf_core::Document,
        span: &emf_core::RecordSpan,
        ih_brush: u32,
    ) {
        let Some(bytes) = document.record_bytes(span) else { return };
        if bytes.len() < 32 {
            return;
        }
        let read_u32 = |at: usize| -> u32 {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let off_bmi = read_u32(16);
        let cb_bmi = read_u32(20);
        let off_bits = read_u32(24);
        let cb_bits = read_u32(28);

        let body = &bytes[32..];
        let bmi_start = off_bmi as usize;
        let bits_start = off_bits as usize;
        let cb_bmi = cb_bmi as usize;
        let cb_bits = cb_bits as usize;
        let checked = |start: usize, len: usize| -> bool {
            start >= 32 && start.saturating_sub(32) + len <= body.len()
        };
        if !checked(off_bmi as usize, cb_bmi) || !checked(off_bits as usize, cb_bits) {
            return;
        }
        let bmi = &body[bmi_start - 32..bmi_start - 32 + cb_bmi];
        let bits = &body[bits_start - 32..bits_start - 32 + cb_bits];
        // The reference dedup key spans cb_bits bytes from the BMI start.
        let dedup_end = (bmi_start - 32 + cb_bits).min(body.len());
        let dedup_key = body[bmi_start - 32..dedup_end].to_vec();

        let image_id = match self.image_library.get(&dedup_key) {
            Some(id) => *id,
            None => {
                let id = self.next_image_id;
                self.next_image_id += 1;
                self.image_library.insert(dedup_key, id);
                // 1-bpp brushes recolor through the device context.
                let mut dib = bmi.to_vec();
                dib.extend_from_slice(bits);
                let mono_palette = if !bmi.is_empty() && bmi[14] == 1 {
                    let text = self.dc.text_color;
                    let bk = self.dc.bk_color;
                    Some([
                        [text.red, text.green, text.blue, 255],
                        [bk.red, bk.green, bk.blue, 255],
                    ])
                } else {
                    None
                };
                let Some(decoded) = raster::decode_dib(&dib, None, mono_palette) else {
                    return;
                };
                let raster::Decoded::Rgba { width, height, rgba } = decoded else { return };
                let Ok(mut png) = raster::encode_png_rgba8(width as u32, height as u32, &rgba)
                else {
                    return;
                };
                // The same fmem modulo-3 zero padding as href_for.
                let rem = png.len() % 3;
                if rem != 0 {
                    png.extend(std::iter::repeat_n(0u8, 3 - rem));
                }
                self.items.push(Item::DefineRasterPattern(RasterPatternDef {
                    id,
                    raster: Raster { format: RasterFormat::Png, payload: png },
                    width: i64::from(width),
                    height: i64::from(height),
                }));
                id
            }
        };

        self.object_table.store(
            ih_brush,
            GdiObject::Brush(Brush {
                style: 8,
                color: self.dc.brush.color,
                hatch: 0,
                image_id: Some(image_id),
            }),
        );
    }
}

enum ArcMode {
    Arc,
    Chord,
    Pie,
}

fn resolve_cap(style: u32) -> Option<LineCap> {
    match style & 0x0F00 {
        0x0000 => Some(LineCap::Round),
        0x0100 => Some(LineCap::Square),
        0x0200 => Some(LineCap::Butt),
        _ => None,
    }
}

fn resolve_join(style: u32, miter_limit: f64) -> Option<LineJoin> {
    match style & 0xF000 {
        0x0000 => Some(LineJoin::Round),
        0x1000 => Some(LineJoin::Bevel),
        0x2000 => Some(LineJoin::Miter { limit: miter_limit }),
        _ => None,
    }
}

fn hatch_style_of(hatch: u32) -> HatchStyle {
    match hatch {
        0 => HatchStyle::Horizontal,
        1 => HatchStyle::Vertical,
        2 => HatchStyle::ForwardDiagonal,
        3 => HatchStyle::BackDiagonal,
        4 => HatchStyle::Cross,
        _ => HatchStyle::DiagonalCross,
    }
}

/// The curve triplets of a calibrated bezier point list, C-joined into the
/// geometry.
fn push_bezier_triplets(geometry: &mut PathGeometry, ctrls: &[(f64, f64)]) {
    for chunk in ctrls.chunks_exact(3) {
        let [(x1, y1), (x2, y2), (x3, y3)] = chunk[..] else { continue };
        geometry.push(Segment::CubicTo { x1, y1, x2, y2, x: x3, y: y3 });
    }
}

fn points_of(points: &[emf_core::Point]) -> Vec<(i32, i32)> {
    points.iter().map(|p| (p.x, p.y)).collect()
}

fn points16_of(points: &[emf_core::Point16]) -> Vec<(i32, i32)> {
    points.iter().map(|p| (i32::from(p.x), i32::from(p.y))).collect()
}

fn color_of(color: &emf_core::ColorRef) -> Color {
    Color { red: color.red, green: color.green, blue: color.blue }
}

fn font_from_record(font: &emf_core::Font) -> Font {
    match font {
        emf_core::Font::LogFont(lf) => font_from_logfont(lf),
        emf_core::Font::Panose(panose) => font_from_logfont(&panose.log_font),
        emf_core::Font::ExDv(ex_dv) => font_from_logfont(&ex_dv.log_font_ex.log_font),
    }
}

fn font_from_logfont(lf: &emf_core::LogFont) -> Font {
    // The face-name quirk: the full 32-char buffer converts; invalid
    // surrogate pairs anywhere (including the unused tail) yield no face.
    let mut raw = Vec::with_capacity(64);
    for unit in &lf.face_name {
        raw.extend_from_slice(&unit.to_le_bytes());
    }
    let face_name = if !valid_utf16le_pairs(&raw) {
        None
    } else {
        let decoded = glyph::decode_utf16_with_question_mark(&lf.face_name);
        let cut = decoded.find('\0').unwrap_or(decoded.len());
        let decoded = &decoded[..cut];
        if decoded.is_empty() {
            None
        } else {
            Some(decoded.to_string())
        }
    };
    // The C remainder preserves the dividend sign.
    let escapement = {
        let value = i64::from(lf.escapement);
        let rem = value - (value / 3600) * 3600;
        rem as i32
    };
    Font {
        face_name,
        height: lf.height,
        weight: lf.weight,
        italic: lf.italic != 0,
        underline: lf.underline != 0,
        strikeout: lf.strike_out != 0,
        escapement,
        charset: lf.char_set,
    }
}

/// iconv-style UTF-16LE validation: any lone surrogate pair kills the face.
fn valid_utf16le_pairs(bytes: &[u8]) -> bool {
    if bytes.len() % 2 != 0 {
        return false;
    }
    for pair in bytes.chunks_exact(2) {
        let code = u16::from_le_bytes([pair[0], pair[1]]);
        if (0xD800..=0xDFFF).contains(&code) {
            return false;
        }
    }
    true
}

fn cp1252_to_utf8(bytes: &[u8]) -> String {
    const HIGH: [Option<char>; 32] = [
        Some('€'), None, Some('‚'), Some('ƒ'), Some('„'), Some('…'), Some('†'), Some('‡'),
        Some('ˆ'), Some('‰'), Some('Š'), Some('‹'), Some('Œ'), None, Some('Ž'), None,
        None, Some('‘'), Some('’'), Some('“'), Some('”'), Some('•'), Some('–'), Some('—'),
        Some('˜'), Some('™'), Some('š'), Some('›'), Some('œ'), None, Some('ž'), Some('Ÿ'),
    ];
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x80..=0x9F => match HIGH[(b - 0x80) as usize] {
                Some(c) => out.push(c),
                None => out.push('?'),
            },
            _ => out.push(b as char),
        }
    }
    out
}

fn rgb_hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)
}

fn c_int_div(a: i64, b: i64) -> f64 {
    (a as f64 / b as f64).trunc()
}

fn floor_div(a: i64, b: i64) -> i64 {
    // Ruby Integer#/ floors.
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}
