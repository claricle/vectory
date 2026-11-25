//! The drawing: canvas, preamble anchor, and the ordered display list.
//!
//! A drawing is a deterministic sequence of items — transform groups,
//! definitions (clips, raster patterns, hatch patterns), and drawn shapes —
//! in document order. Everything a serializer needs is carried; nothing
//! format-specific is.
#![allow(dead_code)]

use crate::geometry::{Matrix, PathGeometry};
use crate::paint::{Arrangement, Color, HatchStyle, PathPaint};
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
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub paint: PathPaint,
    pub arrangement: Arrangement,
    pub body: DrawBody,
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

/// Path geometry plus its authoring arrangement.
#[derive(Clone, Debug, PartialEq)]
pub struct PathDraw {
    pub geometry: PathGeometry,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageDraw {
    pub raster: Raster,
    pub arrangement: ImageArrangement,
}

/// One display-list item.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Open a transform group; all content until the matching
    /// [`Item::GroupClose`] is transformed.
    GroupOpen {
        transform: Matrix,
    },
    /// Close the innermost open transform group.
    GroupClose,
    DefineClip(ClipDef),
    DefineRasterPattern(RasterPatternDef),
    DefineHatchPattern(HatchPatternDef),
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
