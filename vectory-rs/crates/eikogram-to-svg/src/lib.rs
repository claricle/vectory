//! # eikogram-to-svg — the VIM → SVG-model transformer
//!
//! Expresses a [`eikogram::Drawing`] through the `vectory` SVG model
//! (`vectory::Document`), serializable with the existing
//! [`vectory::Profile::EmfsvgSvgBuilder`] profile. The transformer invents no
//! serializer: it builds the SVG tree; the SVG model's own profile emits the
//! bytes.
//!
//! The emission semantics ported here are the reference SVG builder's
//! rules (emfsvg's svg_builder + the renderer's element families), one to
//! one:
//!
//! * number formatting — C `printf("%.4f")` semantics on the actual IEEE-754
//!   bits, half-even at the fourth decimal (the reference renderer's
//!   `FormatHelpers`), plus the shortest-round-trip float form of the
//!   attribute-map families;
//! * element families — calibrated/placed images, region and contour paths,
//!   deferred paths, text runs with their attribute sequence, clip and
//!   pattern definitions, transform groups;
//! * the documented quirk set — the wide tag close of form-closed paths, the
//!   whitespace-collapsed quirk sites (the SVG model declares those
//!   non-representable and its fidelity tests normalize them on both
//!   sides), and the degenerate-y-repair preamble literal.
//!
//! # Boundaries
//!
//! VIM vocabulary in, SVG tree out. The transformer is the only place where
//! the drawing model and the SVG model meet (the model-driven law); EMF
//! concepts never appear here.

mod emit;
mod number;

pub use emit::{drawing_to_document, drawing_to_svg};

use eikogram::Drawing;

/// Transforms a VIM drawing into SVG bytes via the SVG model's
/// `EmfsvgSvgBuilder` profile.
///
/// # Errors
///
/// Returns the serialization error if the built tree fails to serialize
/// (well-formed VIM drawings do not).
pub fn drawing_to_svg_string(drawing: &Drawing) -> Result<String, vectory::Error> {
    let document = drawing_to_document(drawing);
    Ok(vectory::serialize(
        &document,
        vectory::Profile::EmfsvgSvgBuilder,
    ))
}
