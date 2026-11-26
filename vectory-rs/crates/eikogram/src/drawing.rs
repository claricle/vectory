//! The drawing: canvas, preamble anchor, and the ordered display list.
//!
//! A drawing is a deterministic sequence of items — transform groups,
//! definitions (clips, raster patterns, hatch patterns), and drawn shapes —
//! in document order. Everything a serializer needs is carried; nothing
//! format-specific is.
#![allow(dead_code)]

use crate::geometry::{Matrix, PathGeometry};
use crate::paint::{Arrangement, Color, DevicePaint, HatchStyle, PathPaint};
use crate::raster::Raster;
use crate::text::TextDraw;

/// The drawing canvas: output dimensions (y-down document pixels) and the
/// preamble anchor applied to all content.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    /// The preamble anchor, resolved into output units by the producing
    /// transformer.
    pub preamble: Preamble,
    /// The canvas's y-range was sign-degenerate and repaired by zeroing the
    /// preamble; the serializer emits the zeroed anchor verbatim (a reference
    /// repair whose literal form is carried by the flag, not by numbers).
    pub degenerate_y_repair: bool,
    /// The canvas namespace is also declared under its alias.
    pub alias_namespace: bool,
    /// The integer device-units-per-canvas-unit mapping applied to record
    /// coordinates (the mapping prelude a lowering emits when it is not 1;
    /// the default 1 maps coordinates directly).
    pub scale: u64,
}

/// The preamble anchor: a translate applied to all content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preamble {
    pub tx: f64,
    pub ty: f64,
}

/// A clip definition: an id plus the clip shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipDef {
    pub id: u32,
    pub shape: ClipShape,
}

/// The clip shape: bounds (emitted as the four-corner contour) or carried
/// path geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipShape {
    Bounds(PathGeometry),
    Path(PathGeometry),
}

/// A raster pattern definition: an embedded image plus the pattern wrapper
/// that fills reference it. Width/height are the image's pixel dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterPatternDef {
    pub id: u32,
    pub raster: Raster,
    pub width: i64,
    pub height: i64,
}

/// A hatch pattern definition: the tile geometry drawn in the hatch colour
/// over an optional background.
#[derive(Clone, Debug, PartialEq)]
pub struct HatchPatternDef {
    pub id: String,
    pub style: HatchStyle,
    pub foreground: Color,
    /// `None` leaves the tile unpainted (the transparent-background mode).
    pub background: Option<Color>,
}

/// A drawn shape: geometry plus paint plus the authoring arrangement.
///
/// The remaining fields carry the run's resolved lowering facts, each a
/// drawing-semantic value the producing transformer resolved (a device
/// anchor, a finer device mapping, a rectangular clip bracket around the run
/// alone, and the run's carried provenance payload); the defaults are the
/// unqualified form.
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub paint: PathPaint,
    pub arrangement: Arrangement,
    pub body: DrawBody,
    /// A device-space local origin the run's coordinates are relative to
    /// (resolved device units; the lowering brackets the run with the
    /// origin change).
    pub origin: Option<DevicePoint>,
    /// The run's device-unit scale override (a finer mapping than the
    /// canvas scale; the lowering brackets the run with the mapping
    /// change).
    pub scale: Option<u64>,
    /// A rectangular clip bracket wrapped around this run alone (resolved
    /// device units; the lowering opens state, intersects the rectangle,
    /// and closes state around the run).
    pub clip_bracket: Option<DeviceRect>,
    /// The run joins the previous run's clip bracket (identical resolved
    /// bounds and complete-draw facts, adjacent in the list), so the
    /// lowering emits no state records of its own for this run.
    pub clip_join: bool,
    /// A world transform the run applies to its own geometry (a carried
    /// element transform; the lowering selects it after the run's state
    /// push).
    pub world_transform: Option<Matrix>,
    /// The run's drawing provenance payload, in the producing provenance's
    /// exact carried values.
    pub provenance: Option<Provenance>,
    /// The run's resolved device paint (the pen, brush, and miter limit the
    /// run selects).
    pub device_paint: Option<DevicePaint>,
}

impl Default for Draw {
    fn default() -> Self {
        Draw {
            paint: PathPaint::default(),
            arrangement: Arrangement::RegionFillFirst,
            body: DrawBody::Shape(ShapeDraw {
                kind: ShapeKind::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
            }),
            origin: None,
            scale: None,
            clip_bracket: None,
            clip_join: false,
            world_transform: None,
            provenance: None,
            device_paint: None,
        }
    }
}

/// A device-space point (resolved record coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevicePoint {
    pub x: i64,
    pub y: i64,
}

/// A device-space rectangle (resolved record coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceRect {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

/// A drawn run's carried provenance payload. Rectangular provenance keeps
/// the producing provenance's exact textual attribute values (they are not
/// derivable from the parsed numbers); colour and media provenance carry
/// their resolved values.
#[derive(Clone, Debug, PartialEq)]
pub enum Provenance {
    /// A rectangle's textual attribute values plus its reference hex fill.
    RectText(BareRect),
    /// A stroke's resolved colour (the reference hex form is derived).
    StrokeColor(Color),
    /// A JPEG media payload (no carried values).
    Jpeg,
}

/// Drawn geometry, one variant per reference shape family.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawBody {
    Shape(ShapeDraw),
    Path(PathDraw),
    Text(TextDraw),
    Image(ImageDraw),
    /// The provenance shape: a rectangle whose attribute values are carried
    /// in the producing provenance's exact textual form (a round-trip
    /// contract that predates the resolution rules).
    BareRect(BareRect),
}

/// The provenance rectangle: attribute values in carried textual form.
#[derive(Clone, Debug, PartialEq)]
pub struct BareRect {
    pub x: String,
    pub y: String,
    pub width: String,
    pub height: String,
    /// The fill colour in its reference hex form.
    pub fill: String,
}

/// Rect-family geometry in output space, with the y-repair already applied by
/// the producing transformer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    RoundedRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: f64,
        ry: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeDraw {
    pub kind: ShapeKind,
}

/// Path geometry plus its authoring arrangement and, when resolved by the
/// producing transformer, the drawing program the geometry expresses (which
/// record family sequence a lowering emits; `None` leaves the program to the
/// lowering's own rules).
#[derive(Clone, Debug, PartialEq)]
pub struct PathDraw {
    pub geometry: PathGeometry,
    pub program: Option<PathProgram>,
}

/// The drawing program a resolved path run expresses: one variant per
/// distinct record sequence a lowering emits, over the run's carried
/// geometry (contour breaks are its MoveTo segments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathProgram {
    /// One closed contour region.
    Polygon,
    /// One open contour run.
    Polyline,
    /// Cubic contours, each drawn after an explicit current-position move.
    CurvesPerContour,
    /// A line run continued from the carried current position.
    LineFromCurrent,
    /// A cubic run continued from the carried current position.
    CurveFromCurrent,
    /// A multi-contour region run.
    MultiContour { closed: bool },
    /// A deferred figure set, painted by a closing action (`pending` leaves
    /// the figure unpainted; `closed_regions` carries the contours as one
    /// multi-contour region inside the figure instead of as segments).
    Figure {
        paint: FigurePaint,
        pending: bool,
        closed_regions: bool,
    },
    /// Absolute segments, each from its own move.
    Segments,
    /// A solid rectangle painted directly (its extent is the raw
    /// difference, unscaled by any origin shift).
    Blit { extent: DevicePoint },
    /// Cubic contours in one poly-cubic record, after an explicit
    /// current-position move.
    PolyBezier { prefix: Option<DevicePoint> },
    /// A native circular arc form, fully resolved in device units.
    Circular(ArcForm),
}

/// The painting action that closes a deferred figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FigurePaint {
    Fill,
    Stroke,
    StrokeFill,
}

/// A native circular arc family: the arc's device-space box, endpoint
/// radials, the entry and exit moves, and the sweep direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcForm {
    pub kind: ArcKind,
    /// The sweep runs clockwise (the direction a lowering encodes).
    pub clockwise: bool,
    pub prefix: DevicePoint,
    pub finish: DevicePoint,
    pub arc_box: DeviceRect,
    pub start: DevicePoint,
    pub end: DevicePoint,
}

/// The native circular arc family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcKind {
    Arc,
    Chord,
    Pie,
}

/// Placement of an embedded image, per authoring family. All values are
/// resolved output-space numbers; the arrangement selects the attribute
/// order and numeric forms.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageArrangement {
    /// The calibrated family: width, height, x, y in mapped output units.
    Calibrated {
        width: f64,
        height: f64,
        x: f64,
        y: f64,
    },
    /// The placed family: x, y, width, height as authored floats.
    Placed {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// The region-bounds family: integer x/width/height from the carried
    /// region, but the float mapped y (the reference maps only the y).
    BodyBounds {
        x: i64,
        y: f64,
        width: i64,
        height: i64,
    },
    /// The device-resolved family: the source rectangle, destination
    /// point, and destination extent, all final device values.
    Device {
        bounds: DeviceRect,
        destination: DevicePoint,
        extent: DevicePoint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageDraw {
    pub raster: Raster,
    pub arrangement: ImageArrangement,
}

/// One display-list item.
#[allow(clippy::large_enum_variant)] // the drawn run dominates by design
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Open a transform group; all content until the matching
    /// [`Item::GroupClose`] is transformed.
    GroupOpen {
        transform: Matrix,
    },
    /// Close the innermost open transform group.
    GroupClose,
    /// Push the canvas state (no transform change); the matching
    /// [`Item::StatePop`] restores it.
    StatePush,
    /// Pop the innermost pushed canvas state.
    StatePop,
    /// Define a clip: an id plus the clip shape.
    DefineClip(ClipDef),
    DefineRasterPattern(RasterPatternDef),
    DefineHatchPattern(HatchPatternDef),
    /// Intersect the canvas clip with a device-space rectangle.
    ClipRectangle(DeviceRect),
    /// Reset the canvas clip to the default (empty) region.
    ClipReset,
    /// Select the current drawing path as the canvas clip.
    ClipFromPath,
    Draw(Draw),
}

/// The drawing: canvas plus ordered display list.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub canvas: Canvas,
    pub items: Vec<Item>,
}

impl Drawing {
    /// A drawing on the given canvas with an empty display list.
    #[must_use]
    pub fn new(canvas: Canvas) -> Self {
        Drawing {
            canvas,
            items: Vec::new(),
        }
    }
}
