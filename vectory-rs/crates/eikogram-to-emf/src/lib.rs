//! # eikogram-to-emf — the VIM → EMF-model transformer
//!
//! Lowers a [`eikogram::Drawing`] onto the emf-core model and writes the
//! bytes through emf-core's checked pipeline (claricle/emf#90; the reverse
//! lane of the conversion hub). The semantics are the Ruby reference
//! translation's exact choices (emfsvg's Translation::EmfRenderer +
//! handlers + shared GDI lifecycle), one to one:
//!
//! * the mapping prelude and header construction (bounds from the preamble
//!   anchor, the 96-dpi frame, the truncated device fields);
//! * the per-run GDI lifecycle — miter, pen, brush, clip bracket, drawn
//!   records, deletes — in the reference's exact order;
//! * the drawing-program families a run expresses (poly families, figures,
//!   native circular arcs, direct blits), each one record sequence;
//! * text runs with their font identity bytes, alignment, and the UTF-16
//!   payload layout;
//! * image and pattern payloads through the DIB/PNG machinery;
//! * the drawing-provenance comments (round-trip hints), re-encoded with
//!   the record fingerprint.
//!
//! ## emf-core construction
//!
//! The pinned emf-core document model is parse-only (fields and payload
//! lengths are crate-private), so the lowering builds typed
//! `emf_core::RecordData` values and the `wire` module frames them for
//! `emf_core::parse`; every document is validated by parse and finalized by
//! `emf_core::serialize` before leaving this crate. When emf-core grows a
//! public builder (claricle/emf), the framing module shrinks to that call.
//!
//! ## Boundaries
//!
//! The transformer is the only place the VIM and the EMF model meet. The
//! VIM carries no EMF concepts; this crate never parses SVG (the parity
//! gate, in tests, composes this crate with `svg-to-eikogram`).
//!
//! # Errors
//!
//! [`drawing_to_emf`] fails on raster payloads the DIB/PNG machinery
//! refuses or records the wire size field cannot express.

mod fingerprint;
mod hint;
mod lower;
mod raster;
mod wire;

pub use lower::{drawing_to_emf, LoweringFailure};
