//! Text runs: resolved placement, font identity, anchors, and content.
#![allow(dead_code)]

use crate::paint::Color;

/// Horizontal anchor of a text run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

/// Vertical placement of a text run: at the carried y, or raised by the
/// resolved font height fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselineMode {
    AtY,
    Raised,
}

/// A text run in output space. Every field is resolved: the producing
/// transformer applies its coordinate mapping, font scaling and anchor
/// arithmetic before constructing the run, so serialization needs no source
/// knowledge.
#[derive(Clone, Debug, PartialEq)]
pub struct TextDraw {
    pub x: f64,
    pub y: f64,
    /// The resolved font height (output units, already absolute).
    pub font_height: f64,
    /// The font family identity, when the run carries one. When absent, the
    /// family attribute is omitted (the reference "font not carried" quirk).
    pub face: Option<String>,
    /// The run colour (the canvas text colour at authoring time).
    pub color: Color,
    /// Whole-degree rotation, when the font carries an escapement. The
    /// raise-then-rotate emission order is the serializer's.
    pub rotation: Option<i32>,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    /// The font weight; zero weights are omitted at emission.
    pub weight: i64,
    pub anchor: TextAnchor,
    pub baseline: BaselineMode,
    pub content: String,
    /// The recovered native font facts of a canonical rotated run: the
    /// escapement (tenths of a degree, with the zero-angle quirk), and the
    /// recovered native height and baseline (`None` keeps the run's own
    /// resolved values). Carried when the producing provenance recovered
    /// them; a run with a recovered native height maps under a finer
    /// device scale.
    pub native: Option<TextNative>,
    /// The run continues the previous run's font state (identical selected
    /// font facts and adjacency), so its lowering reuses that state instead
    /// of reselecting it.
    pub font_continuation: bool,
    /// The run's resolved device facts: the declared bounds, the reference
    /// point, the selected font identity bytes, and the alignment mode.
    pub device_run: Option<TextRun>,
}

/// A text run's resolved device facts, carried final by the producing
/// transformer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRun {
    /// The declared run bounds (device units).
    pub bounds: crate::drawing::DeviceRect,
    /// The reference point (device units).
    pub reference: crate::drawing::DevicePoint,
    /// The selected font identity bytes (the full logical-font payload).
    pub font: Vec<u8>,
    /// The alignment mode.
    pub align: u32,
}

/// The recovered native font facts of a canonical rotated text run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextNative {
    pub escapement: i32,
    pub height: Option<f64>,
    pub y: Option<f64>,
}
