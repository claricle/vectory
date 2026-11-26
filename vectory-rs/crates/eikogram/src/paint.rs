//! Painting vocabulary: fills, strokes, hatch fills, resolved device paint,
//! and the arrangement a drawn shape's paint attaches to its geometry with.
#![allow(dead_code)]

use crate::drawing::DevicePoint;

/// An opaque sRGB colour carried channel-wise; alpha lives outside the
/// reference paint model (colour emission is channel-hex).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Color { red, green, blue }
    }

    #[must_use]
    pub const fn black() -> Self {
        Color::new(0, 0, 0)
    }

    #[must_use]
    pub const fn white() -> Self {
        Color::new(255, 255, 255)
    }
}

/// A hatch pattern's line layout (six reference cross-hatch families).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HatchStyle {
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackDiagonal,
    Cross,
    DiagonalCross,
}

/// A fill: no paint, a solid colour, or a reference to a defined pattern.
#[derive(Clone, Debug, PartialEq)]
pub enum FillStyle {
    None,
    Solid {
        color: Color,
    },
    /// Fill through a defined raster pattern (`url(#id)` in reference
    /// emission — the id is carried, the `url(#…)` form is the serializer's).
    PatternRef {
        id: String,
    },
    /// Fill through a defined hatch pattern (pattern-reference fills in the
    /// attr-map authoring family).
    HatchRef {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub style: FillStyle,
}

impl Fill {
    #[must_use]
    pub fn none() -> Self {
        Fill {
            style: FillStyle::None,
        }
    }

    #[must_use]
    pub fn solid(color: Color) -> Self {
        Fill {
            style: FillStyle::Solid { color },
        }
    }
}

/// A resolved stroke width. Reference authoring resolves the width to either
/// the literal one-pixel form or a scaled number at paint time; the choice is
/// carried because it changes the emitted text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
    /// The literal one-pixel width.
    OnePixel,
    /// The legacy numeric one form (a round-trip provenance variant).
    OneLegacy,
    /// A scaled width in output units.
    Scaled(f64),
    /// A raw count width (the attribute-map family's unscaled integer).
    Count(i64),
    /// The zero width emitted for the doubly-unpainted quirk.
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Round,
    Square,
    Butt,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LineJoin {
    Round,
    Bevel,
    Miter { limit: f64 },
}

/// A resolved dash pattern in output units (each entry is a scaled length).
#[derive(Clone, Debug, PartialEq)]
pub struct Dash(pub Vec<f64>);

/// A stroke, resolved by the producing transformer. The reference authoring
/// resolves the null-pen quirk against the fill at paint time, so the two
/// fallback forms are carried as resolved variants.
#[derive(Clone, Debug, PartialEq)]
pub enum Stroke {
    /// A painted stroke: colour, resolved width, and the line-style extras
    /// in their attachment order (dash, cap, join).
    Solid {
        color: Color,
        width: Width,
        dash: Option<Dash>,
        cap: Option<LineCap>,
        join: Option<LineJoin>,
    },
    /// The doubly-unpainted quirk: stroke suppressed with a zero width.
    NoneWithZeroWidth,
    /// The unpainted-pen quirk: the stroke falls back to the fill colour at
    /// the one-pixel form (the colour is carried resolved).
    FromFill { color: Color },
}

/// The paint attached to a drawn shape: fill, stroke, and an optional clip
/// reference (a [`crate::Item::DefineClip`] id).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PathPaint {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub clip: Option<u32>,
    /// The stroke fell back to the fill colour (the unpainted-pen quirk that
    /// strokes with the fill colour at one pixel). Carried resolved: when
    /// set, the stroke colour is the fill colour and the width is the
    /// one-pixel form.
    pub stroke_from_fill: bool,
}

/// The resolved device paint a drawn run selects. Every value is final:
/// the producing transformer resolved units, origin shifts, and style bits,
/// so a lowering emits them as the run's selected state without further
/// arithmetic. The pen and brush are both always carried for a run that
/// selects state (a null pen or brush carries the null form); the miter
/// limit rides along when the run sets one.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DevicePaint {
    pub pen: Option<DevicePen>,
    pub brush: Option<DeviceBrush>,
    /// The miter limit value the run sets, in its wire form.
    pub miter_limit: Option<f32>,
}

/// A resolved device pen: the style bits, the width in device units, and
/// the colour (a null pen carries the null colour form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevicePen {
    pub style: u32,
    pub width: DevicePoint,
    pub color: Color,
}

/// The resolved device brush: a solid (or null) colour form, or a reference
/// to a defined raster pattern the run selects (definitions keep their
/// handles; a referenced brush is not deleted with the run).
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceBrush {
    Solid { style: u32, color: Color },
    Pattern { id: String },
}

/// How a drawn shape's paint attaches to its geometry. Each value is a
/// distinct authoring family with its own attachment order and tag-closing
/// form; the enum carries the family so serializers need no format or record
/// knowledge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrangement {
    /// A closed region painted stroke-first (filled contour authoring).
    RegionStrokeFirst,
    /// A closed region painted fill-first (multi-region authoring).
    RegionFillFirst,
    /// An open contour painted stroke-only, with an explicit unfilled
    /// fallback; `true` adds the leading-space variant of the fallback.
    ContourOpen { leading_fill_space: bool },
    /// A deferred path: geometry accumulates, paint lands at the closing
    /// action (fill-first when both paints are present).
    DeferredPath,
    /// A closed form painted stroke-then-fill with the wide tag close.
    FormClosed,
    /// The attribute-map family: fill, stroke extras, then geometry last.
    AttrMap,
    /// A solid-filled rectangle expressed as a path, style-attribute first.
    SolidRectFill,
    /// The bare shape emission: geometry verbatim from provenance values,
    /// single colour fill, no stroke, no trailing newline.
    BareShape,
}
