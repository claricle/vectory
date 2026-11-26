//! # eikogram — the Vector Image Model (VIM)
//!
//! The format-neutral vector drawing super-model (claricle/vectory#102): one
//! internal model for DRAWING semantics, of which file formats are encodings.
//! SVG is one serialization of it (via the `eikogram-to-svg` transformer into the
//! `vectory` SVG model); EMF is another encoding (the emf workspace's
//! transformer); pixel rasterization is a third lane.
//!
//! ## What the VIM holds
//!
//! Format-free drawing semantics only — no EMF record names, no SVG element
//! names, no PS operators:
//!
//! * **canvas state** — output dimensions, the preamble anchor applied to all
//!   content, and transform groups opened/closed around content;
//! * **geometry** — paths (move/line/cubic arc segments over f64 points,
//!   y-down document pixels), rects, rounded rects, ellipses;
//! * **painting** — fills (solid/none/pattern/hatch references), strokes
//!   (resolved width, dash, caps, joins, miter limit), clip references;
//! * **text runs** — resolved placement, font identity, anchors, content;
//! * **raster references** — embedded images as opaque payload handles (the
//!   pixel data itself belongs to the raster side, claricle/emf#89).
//!
//! ## Determinism as a design property
//!
//! The parity gate compares FINAL serialized bytes, so the VIM carries
//! everything the serializers need and nothing format-specific:
//!
//! * every coordinate is already in output space (the producing transformer
//!   applies its device/viewport mapping before constructing geometry);
//! * paint carries resolved values (a stroke width is either the literal
//!   one-pixel form or a scaled number — never "depends on the source
//!   units");
//! * identifiers (clip defs, pattern defs) are carried as values so a
//!   producing transformer can reproduce its reference ID stream exactly;
//! * geometry carries its linear join style (tuple vs sequence) because the
//!   two authoring families serialize differently.
//!
//! ## Arrangements
//!
//! A drawn shape's paint attaches to its geometry in an authoring-specific
//! arrangement (a filled region outlines differently than a stroked contour,
//! and a deferred path paints when its closing action lands). The VIM carries
//! that arrangement as an enum value so a serializer needs no format
//! knowledge to emit — see [`paint::Arrangement`].
//!
//! ## Boundaries
//!
//! Pure data; deterministic construction; zero format concepts. EMF mapping
//! lives in the emf workspace; SVG serialization lives in the `vectory`
//! crate; this crate serializes nothing.

pub mod drawing;
pub mod geometry;
pub mod paint;
pub mod raster;
pub mod text;

pub use drawing::{
    ArcForm, ArcKind, BareRect, Canvas, ClipDef, ClipShape, DevicePoint, DeviceRect, Draw,
    DrawBody, Drawing, FigurePaint, HatchPatternDef, ImageArrangement, ImageDraw, Item,
    PathDraw, PathProgram, Preamble, Provenance, RasterPatternDef, ShapeDraw, ShapeKind,
};
pub use geometry::{CoordJoin, Matrix, PathGeometry, Segment};
pub use paint::{
    Arrangement, Color, Dash, DeviceBrush, DevicePaint, DevicePen, Fill, FillStyle, HatchStyle,
    LineCap, LineJoin, PathPaint, Stroke, Width,
};
pub use raster::{Raster, RasterFormat};
pub use text::{BaselineMode, TextAnchor, TextDraw, TextNative, TextRun};
