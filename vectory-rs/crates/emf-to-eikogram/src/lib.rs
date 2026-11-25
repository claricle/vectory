//! # emf-to-eikogram — the EMF-model → VIM transformer
//!
//! Lifts an [`emf_core::Document`] (the parsed EMF metafile model, consumed
//! as the RUST model — never a byte re-parse) into a
//! [`eikogram::Drawing`], the format-neutral Vector Image Model
//! (claricle/vectory#102; the transformer lane of claricle/emf#88).
//!
//! The semantics ported here are the Ruby reference renderer's drawing
//! meaning (emfsvg's renderer + device_context + transform_stack +
//! clip_region + object_table + the EMF record visit rules), one to one:
//!
//! * header analysis — output dimensions, bounds scale, the reference
//!   anchor, the degenerate-y repair;
//! * GDI state — the device-context stack (with the deliberately shared
//!   coordinate state), the object table, the non-popping restore;
//! * the two-pass deferred-path analysis (transforms inside
//!   begin/end-path blocks apply before/after the block's drawing);
//! * per-record drawing rules — coordinate mapping (point_cal), paint
//!   resolution (fill/stroke quirks included), text runs, bitmap records
//!   through the DIB⇄PNG machinery, clip definitions with the BSD-rand ID
//!   stream;
//! * the drawing-hint comments (round-trip provenance), decoded and applied
//!   exactly as the reference does.
//!
//! ## Raster payloads
//!
//! Bitmap records carry DIB payloads; this crate decodes them and re-encodes
//! PNG for the VIM's raster references. The byte-exact DIB/PNG machinery
//! lives in the emf workspace's dib/png lane (claricle/emf#89); until that
//! lane publishes, its decoder/encoder are vendored here under
//! `raster` with provenance comments, to be replaced by the crate
//! dependency.
//!
//! ## Boundaries
//!
//! The transformer is the only place the EMF model and the VIM meet. The VIM
//! crate carries no EMF concepts; this crate never serializes SVG (the
//! parity gate, in tests, composes this crate with `eikogram-to-svg`).
//!
//! # Errors
//!
//! [`render`] fails on header validation failures mirroring the reference
//! renderer's raised errors (non-finite or non-positive dimensions).

mod analysis;
mod fingerprint;
mod glyph;
mod hint;
mod raster;
mod state;
mod transform;

pub use transform::{render, RenderOptions};

use emf_core::ParseFailure;

/// A rendering failure: the reference renderer's raised conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderFailure {
    /// A header dimension or output option was non-finite/non-positive.
    Dimension(String),
    /// The metafile exceeded the bounded record count.
    ResourceLimit { limit: u64 },
}

impl From<ParseFailure> for RenderFailure {
    fn from(failure: ParseFailure) -> Self {
        match failure {
            // Kept for completeness: parse happens before render.
            _ => RenderFailure::Dimension("parse failure".to_string()),
        }
    }
}

impl std::fmt::Display for RenderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderFailure::Dimension(msg) => write!(f, "{msg}"),
            RenderFailure::ResourceLimit { limit } => {
                write!(f, "record count exceeds the bounded limit {limit}")
            }
        }
    }
}

impl std::error::Error for RenderFailure {}
