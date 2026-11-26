//! The lowering: a VIM drawing onto the emf-core model, in the reference
//! translation's exact record order. The VIM items carry every resolved
//! value; this walk emits records mechanically — the shared-GDI run per
//! drawn item, the canvas prelude, and the EOF.

use eikogram::{
    ArcKind, DeviceBrush, DevicePaint, DevicePoint, DeviceRect, Draw, DrawBody, Drawing,
    FigurePaint, Item, PathDraw, PathProgram, Provenance, ShapeKind, TextDraw,
};

use crate::fingerprint::sha256_hex;
use crate::hint;
use crate::raster;
use crate::wire;

/// A lowering failure: a value the wire model cannot express.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoweringFailure {
    /// A raster payload the DIB/PNG machinery refuses.
    Raster(String),
    /// A framed record exceeded the wire size field.
    SizeOverflow,
}

impl std::fmt::Display for LoweringFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoweringFailure::Raster(message) => write!(f, "raster payload: {message}"),
            LoweringFailure::SizeOverflow => write!(f, "record payload exceeds the size field"),
        }
    }
}

/// The record walk state: the monotonically allocating object table
/// (1-indexed, no reuse), the defined pattern handles, and the collected
/// records.
struct Lowerer {
    next_handle: u32,
    pattern_handles: std::collections::HashMap<String, u32>,
    records: Vec<emf_core::RecordData>,
    /// The pending drawing provenance; the next drawn-run record of the
    /// provenance families emits its comment immediately before itself.
    pending_provenance: Option<Provenance>,
    /// The currently selected shared font handle (adjacent runs with
    /// identical font state reuse it; the deletion lands after the last).
    text_font: Option<u32>,
    /// The current figure contour's bounds (its cubic runs carry them).
    figure_bounds: Option<emf_core::Rect>,
}

/// The record families a provenance comment attaches to.
fn provenance_kind(record: &emf_core::RecordData) -> bool {
    matches!(record, emf_core::RecordData::Bitmap(bitmap)
        if matches!(bitmap.record_type(), 76 | 87 | 81))
        || matches!(record, emf_core::RecordData::Polyline { .. })
}

impl Lowerer {
    fn allocate(&mut self) -> u32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }

    fn emit(&mut self, record: emf_core::RecordData) {
        self.records.push(record);
    }

    /// Emits a drawn-run record; when a provenance payload is pending and
    /// the record is of a provenance family, the comment record (carrying
    /// the payload and the record's fingerprint) is emitted first.
    fn emit_drawn(&mut self, record: emf_core::RecordData) {
        if provenance_kind(&record) {
            if let Some(provenance) = self.pending_provenance.take() {
                let bytes = wire::record_bytes(&record);
                let fingerprint = sha256_hex(&bytes);
                let data = hint::encode(&fingerprint, &provenance);
                self.emit(emf_core::RecordData::Comment { data });
            }
        }
        self.emit(record);
    }
}

/// Lowers a drawing into EMF bytes: the header, the record walk, and the
/// wire framing (validated through emf-core's checked parse and finalized
/// by emf-core's serializer).
///
/// # Errors
///
/// Fails on raster payloads the DIB/PNG machinery refuses or on records
/// the wire size field cannot express.
pub fn drawing_to_emf(drawing: &Drawing) -> Result<Vec<u8>, LoweringFailure> {
    let mut lowerer = Lowerer {
        next_handle: 1,
        pattern_handles: std::collections::HashMap::new(),
        records: Vec::new(),
        pending_provenance: None,
        text_font: None,
        figure_bounds: None,
    };
    lowerer.walk(drawing);
    let n_handles = lowerer.next_handle;
    let header = header_bytes(
        drawing,
        u32::try_from(lowerer.records.len() + 1).unwrap_or(u32::MAX),
        n_handles,
    );
    wire::frame_document(&header, &lowerer.records).map_err(|failure| match failure {
        crate::wire::FrameFailure::SizeOverflow | crate::wire::FrameFailure::Unsupported => {
            LoweringFailure::SizeOverflow
        }
        other => LoweringFailure::Raster(other.to_string()),
    })
}

fn header_bytes(drawing: &Drawing, n_records: u32, n_handles: u32) -> Vec<u8> {
    let canvas = &drawing.canvas;
    // The reference header builder: bounds from the preamble anchor (the
    // header translate group's negated offsets), the frame from the 96-dpi
    // device mapping, and the canvas dimensions truncated to integers.
    let bounds = if canvas.degenerate_y_repair {
        // The legacy broken-Y canonical equivalent.
        emf_core::Rect {
            left: -1,
            top: -1,
            right: trunc_i32(canvas.width) - 2,
            bottom: trunc_i32(canvas.height) - 2,
        }
    } else {
        let min_x = -canvas.preamble.tx;
        let min_y = -canvas.preamble.ty;
        emf_core::Rect {
            left: trunc_i32(min_x),
            top: trunc_i32(min_y),
            right: trunc_i32(min_x + canvas.width),
            bottom: trunc_i32(min_y + canvas.height),
        }
    };
    let units_per_pixel = 100.0 * 25.4 / 96.0;
    let frame = emf_core::Rect {
        left: trunc_i32(f64::from(bounds.left) * units_per_pixel),
        top: trunc_i32(f64::from(bounds.top) * units_per_pixel),
        right: trunc_i32(f64::from(bounds.right) * units_per_pixel),
        bottom: trunc_i32(f64::from(bounds.bottom) * units_per_pixel),
    };
    let px_per_mm = 96.0 / 25.4;
    let mut out = Vec::with_capacity(88);
    let word = |out: &mut Vec<u8>, value: u32| out.extend_from_slice(&value.to_le_bytes());
    word(&mut out, 1); // i_type: EMR_HEADER
    word(&mut out, 88); // n_size
    for value in [bounds.left, bounds.top, bounds.right, bounds.bottom] {
        word(&mut out, value as u32);
    }
    for value in [frame.left, frame.top, frame.right, frame.bottom] {
        word(&mut out, value as u32);
    }
    word(&mut out, 0x464D_4520); // signature " EMF"
    word(&mut out, 0x0001_0000); // version
    word(&mut out, 0); // n_bytes (the reference placeholder, written verbatim)
    word(&mut out, n_records);
    out.extend_from_slice(&(n_handles as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // s_reserved
    word(&mut out, 0); // n_description
    word(&mut out, 0); // off_description
    word(&mut out, 0); // n_pal_entries
    word(&mut out, trunc_i32(canvas.width) as u32);
    word(&mut out, trunc_i32(canvas.height) as u32);
    word(&mut out, trunc_i32(canvas.width / px_per_mm) as u32);
    word(&mut out, trunc_i32(canvas.height / px_per_mm) as u32);
    out
}

fn trunc_i32(value: f64) -> i32 {
    // The reference truncates toward zero through Ruby's Float#to_i.
    let truncated = value.trunc();
    if truncated >= 2_147_483_647.0 {
        i32::MAX
    } else if truncated <= -2_147_483_648.0 {
        i32::MIN
    } else {
        truncated as i32
    }
}

impl Lowerer {
    fn walk(&mut self, drawing: &Drawing) {
        let scale = drawing.canvas.scale;
        if scale > 1 {
            // The mapping prelude: anisotropic mode with the
            // device-per-unit factor, so readers divide scaled
            // coordinates back out.
            self.emit(emf_core::RecordData::SetMapMode { map_mode: 8 });
            self.emit(emf_core::RecordData::SetWindowExtEx {
                extent: emf_core::Size {
                    cx: scale as i32,
                    cy: scale as i32,
                },
            });
            self.emit(emf_core::RecordData::SetViewportExtEx {
                extent: emf_core::Size { cx: 1, cy: 1 },
            });
        }
        let base_scale = scale;
        for (index, item) in drawing.items.iter().enumerate() {
            // A shared font state survives only across adjacent
            // continuation runs; anything else ends the lifetime.
            let next_continues = matches!(
                drawing.items.get(index + 1),
                Some(Item::Draw(Draw {
                    body: DrawBody::Text(TextDraw {
                        font_continuation: true,
                        ..
                    }),
                    ..
                }))
            );
            self.item(item, base_scale, next_continues);
        }
        if let Some(handle) = self.text_font.take() {
            self.emit(emf_core::RecordData::DeleteObject { ih_object: handle });
        }
        self.emit(wire::eof());
    }

    fn item(&mut self, item: &Item, base_scale: u64, next_continues: bool) {
        match item {
            Item::GroupOpen { transform } => {
                self.emit(emf_core::RecordData::SaveDc);
                // The reference emits the world transform for every group,
                // including identity ones, so readers rebuild the wrapper.
                self.emit(emf_core::RecordData::SetWorldTransform {
                    xform: xform_of(transform),
                });
            }
            Item::GroupClose => {
                self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 });
            }
            Item::StatePush => self.emit(emf_core::RecordData::SaveDc),
            Item::StatePop => self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 }),
            Item::ClipRectangle(rect) => {
                self.emit(emf_core::RecordData::IntersectClipRect {
                    clip: rect_of(*rect),
                });
            }
            Item::ClipReset => {
                self.emit(emf_core::RecordData::ExtSelectClipRgn {
                    region_mode: 5,
                    region: None,
                });
            }
            Item::ClipFromPath => {
                self.emit(emf_core::RecordData::SelectClipPath { region_mode: 5 });
            }
            Item::DefineRasterPattern(def) => {
                let dib = match raster::pattern_dib(&def.raster, def.width, def.height) {
                    Ok(dib) => dib,
                    Err(message) => {
                        // An undecodable payload contributes no resource;
                        // the lowering reports it through the gate.
                        let _ = message;
                        Vec::new()
                    }
                };
                let handle = self.allocate();
                self.emit(emf_core::RecordData::Bitmap(
                    emf_core::BitmapRecord::CreateDibPatternBrushPt {
                        ih_brush: handle,
                        usage: 0,
                        buffers: raster::brush_buffers(&dib),
                    },
                ));
                self.pattern_handles.insert(def.id.to_string(), handle);
            }
            Item::DefineClip(_) | Item::DefineHatchPattern(_) => {
                // Definitions without device records: referenced clips and
                // hatch fills carry their geometry through the runs that
                // use them.
            }
            Item::Draw(draw) => self.draw(draw, base_scale, next_continues),
        }
    }

    fn draw(&mut self, draw: &Draw, base_scale: u64, next_continues: bool) {
        // The complete-run GDI lifecycle: state push, miter, pen, brush,
        // clip intersect, the run, deletes, state pop.
        if draw.clip_bracket.is_some() {
            self.emit(emf_core::RecordData::SaveDc);
        }
        let paint = draw.device_paint.clone().unwrap_or(DevicePaint {
            pen: None,
            brush: None,
            miter_limit: None,
        });
        if let Some(limit) = paint.miter_limit {
            self.emit(emf_core::RecordData::SetMiterLimit {
                miter_limit: emf_core::Float32::from_f32(limit),
            });
        }
        let mut pen_handle = None;
        if let Some(pen) = &paint.pen {
            let handle = self.allocate();
            self.emit(emf_core::RecordData::CreatePen {
                ih_pen: handle,
                pen_style: pen.style,
                width: point_of(pen.width),
                color: color_of(pen.color),
            });
            self.emit(emf_core::RecordData::SelectObject { ih_object: handle });
            pen_handle = Some(handle);
        }
        let mut brush_handle = None;
        if let Some(brush) = &paint.brush {
            match brush {
                DeviceBrush::Solid { style, color } => {
                    let handle = self.allocate();
                    self.emit(emf_core::RecordData::CreateBrushIndirect {
                        ih_brush: handle,
                        brush_style: *style,
                        color: color_of(*color),
                        brush_hatch: 0,
                    });
                    self.emit(emf_core::RecordData::SelectObject { ih_object: handle });
                    brush_handle = Some(handle);
                }
                DeviceBrush::Pattern { id } => {
                    // Definition brushes stay defined: the run selects the
                    // pattern's handle without deleting it.
                    if let Some(handle) = self.pattern_handles.get(id).copied() {
                        self.emit(emf_core::RecordData::SelectObject { ih_object: handle });
                    }
                }
            }
        }
        if let Some(clip) = draw.clip_bracket {
            self.emit(emf_core::RecordData::IntersectClipRect {
                clip: rect_of(clip),
            });
        }
        let mut scale_bracket = false;
        if let Some(fine) = draw.scale {
            // The finer-mapping bracket: extents stay outside the saved
            // drawing state, so the canvas mapping is restored explicitly.
            scale_bracket = true;
            self.emit(emf_core::RecordData::SaveDc);
            self.emit(emf_core::RecordData::SetWindowExtEx {
                extent: emf_core::Size {
                    cx: fine as i32,
                    cy: fine as i32,
                },
            });
        }
        let mut transform_bracket = false;
        if let Some(matrix) = draw.world_transform {
            if draw.clip_bracket.is_none() && !scale_bracket {
                // The transformed run owns its state push.
                transform_bracket = true;
                self.emit(emf_core::RecordData::SaveDc);
            }
            self.emit(emf_core::RecordData::SetWorldTransform {
                xform: xform_of(&matrix),
            });
        }
        if let Some(origin) = draw.origin {
            self.emit(emf_core::RecordData::SaveDc);
            self.emit(emf_core::RecordData::SetViewportOrgEx {
                origin: point_of(origin),
            });
        }
        self.pending_provenance = draw.provenance.clone();
        match &draw.body {
            DrawBody::Text(_) => self.text(draw, next_continues),
            body => self.body(body, draw),
        }
        self.pending_provenance = None;
        if let Some(origin) = draw.origin {
            self.emit(emf_core::RecordData::SetViewportOrgEx {
                origin: emf_core::Point { x: 0, y: 0 },
            });
            let _ = origin;
            self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 });
        }
        if transform_bracket {
            self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 });
        }
        if scale_bracket {
            self.emit(emf_core::RecordData::SetWindowExtEx {
                extent: emf_core::Size {
                    cx: base_scale as i32,
                    cy: base_scale as i32,
                },
            });
            self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 });
        }
        if let Some(handle) = pen_handle {
            self.emit(emf_core::RecordData::DeleteObject { ih_object: handle });
        }
        if let Some(handle) = brush_handle {
            self.emit(emf_core::RecordData::DeleteObject { ih_object: handle });
        }
        if draw.clip_bracket.is_some() {
            self.emit(emf_core::RecordData::RestoreDc { saved_dc: -1 });
        }
    }

    fn body(&mut self, body: &DrawBody, draw: &Draw) {
        match body {
            DrawBody::Shape(shape) => self.shape(shape.kind, draw),
            DrawBody::Path(path) => self.path(path),
            DrawBody::Image(image) => self.image(image),
            DrawBody::Text(_) | DrawBody::BareRect(_) => {}
        }
    }

    fn shape(&mut self, kind: ShapeKind, draw: &Draw) {
        // Rect-family bounds derive from the resolved geometry; the
        // raster-fill form blits instead of outlining.
        let ShapeKind::Rect {
            x,
            y,
            width,
            height,
        } = kind
        else {
            match kind {
                ShapeKind::RoundedRect {
                    x,
                    y,
                    width,
                    height,
                    rx,
                    ry,
                } => {
                    self.emit_drawn(emf_core::RecordData::RoundRect {
                        bounds: rect_of(DeviceRect {
                            left: x as i64,
                            top: y as i64,
                            right: (x + width) as i64,
                            bottom: (y + height) as i64,
                        }),
                        corner: emf_core::Size {
                            cx: rx as i32,
                            cy: ry as i32,
                        },
                    });
                }
                ShapeKind::Ellipse { cx, cy, rx, ry } => {
                    self.emit_drawn(emf_core::RecordData::Ellipse {
                        bounds: rect_of(DeviceRect {
                            left: (cx - rx) as i64,
                            top: (cy - ry) as i64,
                            right: (cx + rx) as i64,
                            bottom: (cy + ry) as i64,
                        }),
                    });
                }
                ShapeKind::Rect { .. } => unreachable!("handled above"),
            }
            return;
        };
        let right = x + width;
        let bottom = y + height;
        let box_rect = rect_of(DeviceRect {
            left: x as i64,
            top: y as i64,
            right: right as i64,
            bottom: bottom as i64,
        });
        if raster_fill(draw) {
            // The rectangle's null-pen fallback would add a brush-coloured
            // outline; a source-free solid fill blits to keep the
            // fill-only boundary.
            self.emit_drawn(emf_core::RecordData::Bitmap(
                emf_core::BitmapRecord::BitBlt {
                    transfer: emf_core::BitmapTransfer {
                        bounds: box_rect,
                        destination: emf_core::Point {
                            x: x as i32,
                            y: y as i32,
                        },
                        destination_extent: emf_core::Point {
                            x: width as i32,
                            y: height as i32,
                        },
                        source: source_default(),
                        buffers: emf_core::BitmapBuffers::default(),
                    },
                    raster_operation: 0x00F0_0021,
                },
            ));
            return;
        }
        self.emit_drawn(emf_core::RecordData::Rectangle { bounds: box_rect });
    }

    fn path(&mut self, path: &PathDraw) {
        let Some(program) = path.program else {
            return;
        };
        let contours = contours(&path.geometry);
        match program {
            PathProgram::PolyBezier { prefix } => {
                let Some(first) = contours.first() else {
                    return;
                };
                let points = flat_points(first);
                if let Some(prefix) = prefix {
                    self.emit_drawn(emf_core::RecordData::MoveToEx {
                        origin: point_of(prefix),
                    });
                }
                let bounds = bounds_of(&points);
                self.emit_drawn(emf_core::RecordData::PolyBezier { bounds, points });
            }
            PathProgram::Polygon | PathProgram::Polyline => {
                let Some(first) = contours.first() else {
                    return;
                };
                let points = flat_points(first);
                let bounds = bounds_of(&points);
                self.emit_drawn(if matches!(program, PathProgram::Polygon) {
                    emf_core::RecordData::Polygon { bounds, points }
                } else {
                    emf_core::RecordData::Polyline { bounds, points }
                });
            }
            PathProgram::CurvesPerContour => {
                for contour in &contours {
                    let points = flat_points(contour);
                    let Some(first) = points.first().copied() else {
                        continue;
                    };
                    self.emit_drawn(emf_core::RecordData::MoveToEx { origin: first });
                    let rest = points[1..].to_vec();
                    let bounds = bounds_of(&rest);
                    self.emit_drawn(emf_core::RecordData::PolyBezierTo {
                        bounds,
                        points: rest,
                    });
                }
            }
            PathProgram::LineFromCurrent | PathProgram::CurveFromCurrent => {
                let Some(first) = contours.first() else {
                    return;
                };
                let points = flat_points(first);
                let Some(first_point) = points.first().copied() else {
                    return;
                };
                self.emit_drawn(emf_core::RecordData::MoveToEx {
                    origin: first_point,
                });
                let rest = points[1..].to_vec();
                let bounds = bounds_of(&rest);
                self.emit_drawn(if matches!(program, PathProgram::LineFromCurrent) {
                    emf_core::RecordData::PolylineTo {
                        bounds,
                        points: rest,
                    }
                } else {
                    emf_core::RecordData::PolyBezierTo {
                        bounds,
                        points: rest,
                    }
                });
            }
            PathProgram::MultiContour { closed } => {
                let mut counts = Vec::new();
                let mut all = Vec::new();
                for contour in &contours {
                    let points = flat_points(contour);
                    counts.push(points.len() as u32);
                    all.extend(points);
                }
                let bounds = bounds_of(&all);
                self.emit_drawn(if closed {
                    emf_core::RecordData::PolyPolygon {
                        bounds,
                        poly_counts: counts,
                        points: all,
                    }
                } else {
                    emf_core::RecordData::PolyPolyline {
                        bounds,
                        poly_counts: counts,
                        points: all,
                    }
                });
            }
            PathProgram::Figure {
                paint,
                pending,
                closed_regions: _,
            } => {
                self.emit(emf_core::RecordData::BeginPath);
                let mut combined = Vec::new();
                for contour in &contours {
                    let points = flat_points(contour);
                    combined.extend(&points);
                    self.figure_segments(contour);
                }
                self.emit(emf_core::RecordData::EndPath);
                if !pending {
                    let bounds = bounds_of(&combined);
                    self.emit(match paint {
                        FigurePaint::Fill => emf_core::RecordData::FillPath { bounds },
                        FigurePaint::Stroke => emf_core::RecordData::StrokePath { bounds },
                        FigurePaint::StrokeFill => {
                            emf_core::RecordData::StrokeAndFillPath { bounds }
                        }
                    });
                }
            }
            PathProgram::Segments => {
                for contour in &contours {
                    self.figure_segments(contour);
                }
            }
            PathProgram::Blit { extent } => {
                let Some(first) = contours.first() else {
                    return;
                };
                let points = flat_points(first);
                if points.len() < 3 {
                    return;
                }
                let (left, top) = (i64::from(points[0].x), i64::from(points[0].y));
                let (right, bottom) = (i64::from(points[2].x), i64::from(points[2].y));
                self.emit_drawn(emf_core::RecordData::Bitmap(
                    emf_core::BitmapRecord::BitBlt {
                        transfer: emf_core::BitmapTransfer {
                            bounds: rect_of(DeviceRect {
                                left,
                                top,
                                right,
                                bottom,
                            }),
                            destination: emf_core::Point {
                                x: left as i32,
                                y: top as i32,
                            },
                            destination_extent: point_of(extent),
                            source: source_default(),
                            buffers: emf_core::BitmapBuffers::default(),
                        },
                        raster_operation: 0x00F0_0021,
                    },
                ));
            }
            PathProgram::Circular(arc) => {
                self.emit_drawn(emf_core::RecordData::MoveToEx {
                    origin: point_of(arc.prefix),
                });
                self.emit(emf_core::RecordData::SetArcDirection {
                    arc_direction: if arc.clockwise { 2 } else { 1 },
                });
                let record = match arc.kind {
                    ArcKind::Arc => emf_core::RecordData::Arc {
                        bounds: rect_of(arc.arc_box),
                        start: point_of(arc.start),
                        end: point_of(arc.end),
                    },
                    ArcKind::Chord => emf_core::RecordData::Chord {
                        bounds: rect_of(arc.arc_box),
                        start: point_of(arc.start),
                        end: point_of(arc.end),
                    },
                    ArcKind::Pie => emf_core::RecordData::Pie {
                        bounds: rect_of(arc.arc_box),
                        start: point_of(arc.start),
                        end: point_of(arc.end),
                    },
                };
                self.emit_drawn(record);
                self.emit_drawn(emf_core::RecordData::MoveToEx {
                    origin: point_of(arc.finish),
                });
            }
        }
    }

    /// The per-segment figure walk: moves and lines, cubic runs, closes.
    fn figure_segments(&mut self, contour: &eikogram::PathGeometry) {
        for segment in &contour.segments {
            match segment {
                eikogram::Segment::MoveTo { x, y } => {
                    self.emit_drawn(emf_core::RecordData::MoveToEx {
                        origin: emf_core::Point {
                            x: *x as i32,
                            y: *y as i32,
                        },
                    });
                }
                eikogram::Segment::LineTo { x, y } => {
                    self.emit_drawn(emf_core::RecordData::LineTo {
                        origin: emf_core::Point {
                            x: *x as i32,
                            y: *y as i32,
                        },
                    });
                }
                eikogram::Segment::CubicTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    let points = vec![
                        emf_core::Point {
                            x: *x1 as i32,
                            y: *y1 as i32,
                        },
                        emf_core::Point {
                            x: *x2 as i32,
                            y: *y2 as i32,
                        },
                        emf_core::Point {
                            x: *x as i32,
                            y: *y as i32,
                        },
                    ];
                    // The figure's cubic runs carry their contour bounds.
                    let bounds = self.figure_bounds.unwrap_or_else(|| bounds_of(&points));
                    self.emit_drawn(emf_core::RecordData::PolyBezierTo { bounds, points });
                }
                eikogram::Segment::ArcTo { .. } => {}
                eikogram::Segment::Close => self.emit(emf_core::RecordData::CloseFigure),
            }
        }
    }

    fn text(&mut self, draw: &Draw, next_continues: bool) {
        let DrawBody::Text(text) = &draw.body else {
            return;
        };
        let Some(run) = &text.device_run else {
            return;
        };
        if !text.font_continuation {
            let handle = self.allocate();
            let font = font_of(&run.font);
            self.emit(emf_core::RecordData::CreateFontIndirectW(Box::new(
                emf_core::FontRecord {
                    ih_object: handle,
                    font,
                },
            )));
            self.emit(emf_core::RecordData::SelectObject { ih_object: handle });
            self.emit(emf_core::RecordData::SetTextColor {
                color: color_of(text.color),
            });
            self.emit(emf_core::RecordData::SetTextAlign {
                text_align_mode: run.align,
            });
            self.text_font = Some(handle);
        }
        let units = wire::utf16_units(&text.content);
        let chars = text.content.chars().count();
        let text_bytes_len = units.len() * 2;
        let dx_pad = (4 - text_bytes_len % 4) % 4;
        self.emit_drawn(emf_core::RecordData::ExtTextOutW(emf_core::ExtTextOut {
            bounds: rect_of(run.bounds),
            graphics_mode: 2,
            ex_scale: emf_core::Float32::from_f32(1.0),
            ey_scale: emf_core::Float32::from_f32(1.0),
            reference: point_of(run.reference),
            options: 0,
            rectangle: Some(emf_core::Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }),
            text: units,
            dx: Some(vec![0u32; chars]),
            string_padding: Vec::new(),
            dx_padding: vec![0u8; dx_pad],
            dx_first: false,
            string_present: true,
        }));
        if !next_continues {
            if let Some(handle) = self.text_font.take() {
                self.emit(emf_core::RecordData::DeleteObject { ih_object: handle });
            }
        }
    }

    fn image(&mut self, image: &eikogram::ImageDraw) {
        let eikogram::ImageArrangement::Device {
            bounds,
            destination,
            extent,
        } = &image.arrangement
        else {
            return;
        };
        let dib = match raster::image_dib(&image.raster) {
            Ok(dib) => dib,
            Err(message) => {
                // An undecodable payload contributes no image; the parity
                // gate reports the divergence.
                let _ = message;
                Vec::new()
            }
        };
        let (width, height) = raster::dib_dims(&dib);
        self.emit_drawn(emf_core::RecordData::Bitmap(
            emf_core::BitmapRecord::StretchDibits {
                transfer: emf_core::DibTransfer {
                    bounds: rect_of(*bounds),
                    destination: point_of(*destination),
                    source: emf_core::Point { x: 0, y: 0 },
                    source_extent: emf_core::Point {
                        x: width as i32,
                        y: height as i32,
                    },
                    usage: 0,
                    buffers: raster::dib_buffers(&dib),
                },
                raster_operation: 0x00CC_0020,
                destination_extent: point_of(*extent),
            },
        ));
    }
}

/// The rect family's raster-fill form: an unrounded, unclipped, unpainted
/// stroke with a solid fill blits (the fill-only boundary).
fn raster_fill(draw: &Draw) -> bool {
    let is_rect = matches!(
        &draw.body,
        DrawBody::Shape(shape) if matches!(shape.kind, ShapeKind::Rect { .. })
    );
    is_rect
        && draw.clip_bracket.is_none()
        && matches!(
            draw.paint.stroke,
            Some(eikogram::Stroke::NoneWithZeroWidth) | None
        )
        && matches!(
            draw.paint.fill.as_ref().map(|f| &f.style),
            Some(eikogram::FillStyle::Solid { .. })
        )
}

fn xform_of(matrix: &eikogram::Matrix) -> emf_core::XForm {
    emf_core::XForm {
        m11: emf_core::Float32::from_f32(matrix.m11 as f32),
        m12: emf_core::Float32::from_f32(matrix.m12 as f32),
        m21: emf_core::Float32::from_f32(matrix.m21 as f32),
        m22: emf_core::Float32::from_f32(matrix.m22 as f32),
        dx: emf_core::Float32::from_f32(matrix.dx as f32),
        dy: emf_core::Float32::from_f32(matrix.dy as f32),
    }
}

fn color_of(color: eikogram::Color) -> emf_core::ColorRef {
    emf_core::ColorRef {
        red: color.red,
        green: color.green,
        blue: color.blue,
        reserved: 255,
    }
}

fn point_of(point: DevicePoint) -> emf_core::Point {
    emf_core::Point {
        x: point.x as i32,
        y: point.y as i32,
    }
}

fn rect_of(rect: DeviceRect) -> emf_core::Rect {
    emf_core::Rect {
        left: rect.left as i32,
        top: rect.top as i32,
        right: rect.right as i32,
        bottom: rect.bottom as i32,
    }
}

fn source_default() -> emf_core::BitmapSource {
    emf_core::BitmapSource {
        position: emf_core::Point { x: 0, y: 0 },
        transform: emf_core::XForm {
            m11: emf_core::Float32::from_f32(0.0),
            m12: emf_core::Float32::from_f32(0.0),
            m21: emf_core::Float32::from_f32(0.0),
            m22: emf_core::Float32::from_f32(0.0),
            dx: emf_core::Float32::from_f32(0.0),
            dy: emf_core::Float32::from_f32(0.0),
        },
        background_color: emf_core::ColorRef {
            red: 0,
            green: 0,
            blue: 0,
            reserved: 0,
        },
        usage: 0,
    }
}

/// Parses the carried font identity bytes back into the model's logical
/// font (the 92-byte LOGFONT payload).
fn font_of(bytes: &[u8]) -> emf_core::Font {
    let word_at = |at: usize| -> i32 {
        if at + 4 > bytes.len() {
            return 0;
        }
        i32::from_le_bytes(bytes[at..at + 4].try_into().expect("sliced above"))
    };
    let mut face_name = [0u16; 32];
    for (index, unit) in face_name.iter_mut().enumerate() {
        let at = 60 + index * 2;
        if at + 2 <= bytes.len() {
            *unit = u16::from_le_bytes(bytes[at..at + 2].try_into().expect("sliced above"));
        }
    }
    emf_core::Font::LogFont(emf_core::LogFont {
        height: word_at(0),
        width: word_at(4),
        escapement: word_at(8),
        orientation: word_at(12),
        weight: word_at(16),
        italic: bytes.get(20).copied().unwrap_or(0),
        underline: bytes.get(21).copied().unwrap_or(0),
        strike_out: bytes.get(22).copied().unwrap_or(0),
        char_set: bytes.get(23).copied().unwrap_or(0),
        out_precision: bytes.get(24).copied().unwrap_or(0),
        clip_precision: bytes.get(25).copied().unwrap_or(0),
        quality: bytes.get(26).copied().unwrap_or(0),
        pitch_and_family: bytes.get(27).copied().unwrap_or(0),
        face_name,
    })
}

/// Splits a flat geometry into its MoveTo-delimited contours.
fn contours(geometry: &eikogram::PathGeometry) -> Vec<eikogram::PathGeometry> {
    let mut out: Vec<eikogram::PathGeometry> = Vec::new();
    for segment in &geometry.segments {
        if matches!(segment, eikogram::Segment::MoveTo { .. }) || out.is_empty() {
            out.push(eikogram::PathGeometry::new(geometry.join));
        }
        out.last_mut()
            .expect("pushed above")
            .segments
            .push(*segment);
    }
    out
}

fn flat_points(contour: &eikogram::PathGeometry) -> Vec<emf_core::Point> {
    let mut out = Vec::new();
    for segment in &contour.segments {
        match segment {
            eikogram::Segment::MoveTo { x, y } | eikogram::Segment::LineTo { x, y } => {
                out.push(emf_core::Point {
                    x: *x as i32,
                    y: *y as i32,
                });
            }
            eikogram::Segment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                for (px, py) in [(x1, y1), (x2, y2), (x, y)] {
                    out.push(emf_core::Point {
                        x: *px as i32,
                        y: *py as i32,
                    });
                }
            }
            eikogram::Segment::ArcTo { x, y, .. } => out.push(emf_core::Point {
                x: *x as i32,
                y: *y as i32,
            }),
            eikogram::Segment::Close => {}
        }
    }
    out
}

fn bounds_of(points: &[emf_core::Point]) -> emf_core::Rect {
    if points.is_empty() {
        return emf_core::Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
    }
    let (mut left, mut top, mut right, mut bottom) =
        (points[0].x, points[0].y, points[0].x, points[0].y);
    for point in points {
        left = left.min(point.x);
        right = right.max(point.x);
        top = top.min(point.y);
        bottom = bottom.max(point.y);
    }
    emf_core::Rect {
        left,
        top,
        right,
        bottom,
    }
}
