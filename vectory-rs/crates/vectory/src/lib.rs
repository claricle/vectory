//! # vectory — the SVG model in Rust
//!
//! The structured SVG tree (elements, ORDERED attributes, text, CDATA,
//! comments, processing instructions, DOCTYPE, XML declaration) with decode
//! (XML bytes → model) and serialize (model → bytes).
//!
//! **Serializer fidelity is the core feature.** The serializer reproduces the
//! Ruby producers' SVG output byte-for-byte on the corpus. Two explicit
//! profiles exist — never a compromise between them:
//!
//! * [`Profile::RubyXml`] — the vectory writer's engine semantics
//!   (`Leptris::XML.parse(content).root.to_xml`, i.e. libxml2 element
//!   serialization with indent 0). Emits the root subtree only.
//! * [`Profile::EmfsvgSvgBuilder`] — emfsvg's renderer serializer semantics
//!   (`Emfsvg::SvgBuilder`, lib/emfsvg/svg_builder.rb). Emits the whole
//!   document (declaration, prolog, root, epilog) with SvgBuilder's escaping.
//!
//! ## Derived serialization rules
//!
//! Rules below were derived by probing the Ruby producers and are enforced by
//! the test suite (`tests/serializer_rules.rs`, `tests/roundtrip_corpus.rs`,
//! `tests/emfsvg_fidelity.rs`).
//!
//! Shared by both profiles:
//!
//! * R1 — attribute order is model order (source order); never sorted,
//!   canonicalized, or deduplicated beyond XML-required errors. One profile
//!   exception: under `RubyXml`, libxml2 keeps xmlns declarations in the
//!   node's namespace list and emits them before the property list, so
//!   `xmlns`/`xmlns:*` serialize before other attributes at EVERY element
//!   (probe-verified). Under `EmfsvgSvgBuilder` order is pure model order.
//! * R2 — attribute values are always double-quoted.
//! * R3 — an element with no children serializes self-closing as
//!   `<name attrs/>` (no leading space before `/>`); `<a></a>` in the source
//!   also serializes `<a/>`.
//! * R4 — no whitespace is invented: no indentation, no pretty-printing;
//!   whitespace exists only in text nodes and is emitted verbatim.
//! * R5 — CDATA sections serialize verbatim as `<![CDATA[content]]>`; they
//!   are distinct nodes and are never merged with adjacent text.
//! * R6 — comments serialize verbatim as `<!--content-->`.
//! * R7 — processing instructions serialize verbatim as
//!   `<?target content?>` (content includes its leading space when present).
//! * R8 — names (elements, attributes, xmlns declarations) are emitted
//!   lexically; namespace redeclarations and undeclarations (`xmlns:p=""`)
//!   survive byte-identically.
//!
//! Profile `RubyXml` (leptris/libxml2, verified by probe):
//!
//! * R10 — only the root element subtree is emitted; the XML declaration,
//!   DOCTYPE, prolog and epilog comments/PIs are dropped (matches
//!   `document.root.to_xml`).
//! * R11 — text escaping: `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`;
//!   `"` and `'` stay literal; CR → `&#13;`.
//! * R12 — attribute-value escaping: `&` → `&amp;`, `<` → `&lt;`,
//!   `>` → `&gt;`, `"` → `&quot;`; `'` stays literal; TAB → `&#9;`,
//!   LF → `&#10;`, CR → `&#13;`.
//! * R13 — decode normalizes per the XML spec: line ends (CRLF/CR) become LF
//!   in all content; in attribute values literal TAB/LF/CR become spaces,
//!   while character references (`&#9;` `&#10;` `&#13;`) survive as the
//!   characters themselves and are re-escaped per R12 on output.
//!
//! Profile `EmfsvgSvgBuilder` (emfsvg SvgBuilder, source-derived):
//!
//! * R20 — the whole document is emitted: recorded declaration first (the
//!   renderer emits the literal `<?xml version="1.0"  encoding="UTF-8"
//!   standalone="no"?>`, which the model preserves verbatim from the source),
//!   then prolog nodes, root subtree, epilog nodes.
//! * R21 — text escaping: `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;` only;
//!   CR stays literal.
//! * R22 — attribute escaping: `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`,
//!   `"` → `&quot;`; `'`, TAB, LF, CR stay literal.
//! * R23 — no newlines are invented at element boundaries (SvgBuilder's
//!   callers control newlines via `rawln`; in the model those newlines are
//!   text nodes and round-trip verbatim).
//!
//! Known non-representable renderer quirks (bytes inside the tag grammar that
//! XML parsing discards and no conforming model can carry): extra whitespace
//! runs between attributes, space before `=` (`style ="..."`), a space before
//! `/>` (`"...none" />`), and the double space in the renderer's declaration
//! literal — the last IS preserved because the declaration is stored verbatim.
//! The emfsvg fidelity test normalizes exactly these classes on both sides
//! and asserts everything else is byte-equal.
//!
//! ## Namespaces
//!
//! Namespace handling mirrors lib/vectory/svg_namespace_binding.rb
//! semantics: declarations are tracked with scope (shadowing redeclarations
//! and `xmlns:p=""` undeclarations honored), elements resolve through the
//! default namespace, unprefixed attributes have NO namespace, `xml:` binds
//! to the XML namespace, and remapping operates on URIs rather than lexical
//! prefixes. Lexical names are always preserved for serialization.
//!
//! ## Boundaries
//!
//! Zero foreign-format concepts live in this crate — SVG/XML only, per the
//! model-driven law (one model per format; serializers belong to their
//! model). EMF/PNG/PostScript concerns live in their own models.

pub mod class_suffixer;
pub mod css;
pub mod datauri;
pub mod decode;
pub mod dom;
pub mod error;
pub mod graft;
pub mod id_disambiguator;
pub mod input_policy;
pub mod mutation;
pub mod namespace;
pub mod namespace_binding;
pub mod scan;
pub mod serialize;
pub mod svg_document;
pub mod svg_mapping;
pub mod transform;

pub use decode::decode;
pub use dom::{Attribute, Document, Element, Node, Text, XmlDeclaration};
pub use error::Error;
pub use serialize::{serialize, serialize_document, Profile};
pub use transform::{SvgTransform, SvgTransformPipeline};

/// Decode and serialize in one step: XML bytes → model → bytes.
///
/// # Errors
///
/// Returns the decode error if parsing fails.
pub fn roundtrip(content: &str, profile: Profile) -> Result<String, Error> {
    Ok(serialize(&decode(content)?, profile))
}
