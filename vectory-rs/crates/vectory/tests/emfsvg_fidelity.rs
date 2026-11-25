//! Emfsvg renderer-output fidelity: decoding an emfsvg SvgBuilder output and
//! re-serializing with the `EmfsvgSvgBuilder` profile must reproduce the
//! renderer's bytes exactly, except at the enumerated non-representable
//! quirk sites (whitespace inside the tag grammar, which XML parsing
//! discards). Quirk classes live in `normalize_quirks`; everything else —
//! attribute order, attribute values (including the renderer's verbatim
//! typos like `translate(0.0000, 0.00 00)`), CDATA, text, newlines — must
//! match byte-for-byte.
//!
//! Fixtures under `vectory-rs/testdata/emfsvg/` are renderer outputs from
//! the emfsvg golden corpus (claricle/emfsvg spec/fixtures/golden),
//! curated for feature spread: CDATA text, clip paths, embedded images,
//! patterns, gradients, declaration/quirk sites. The corpus's own
//! renderer-produced file (spec/examples/emfsvg/ref.svg) is covered too.

mod common;

use common::{normalize_quirks, repo_spec_dir, testdata_dir};
use std::path::PathBuf;

fn emfsvg_outputs() -> Vec<(PathBuf, String)> {
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let dir = testdata_dir().join("emfsvg");
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "svg") {
            let name = PathBuf::from(path.file_name().expect("file name"));
            let content =
                std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {name:?}: {e}"));
            files.push((name, content));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files.push((
        PathBuf::from("spec/examples/emfsvg/ref.svg"),
        std::fs::read_to_string(repo_spec_dir().join("examples/emfsvg/ref.svg"))
            .expect("corpus renderer output"),
    ));
    files
}

#[test]
fn emfsvg_renderer_outputs_roundtrip_to_renderer_bytes() {
    let files = emfsvg_outputs();
    assert!(files.len() >= 10, "fixture set drifted");
    for (name, source) in &files {
        let document = vectory::decode(source)
            .unwrap_or_else(|e| panic!("{name:?}: decode failed: {e}"));
        let first = vectory::serialize(&document, vectory::Profile::EmfsvgSvgBuilder);
        assert_eq!(
            normalize_quirks(source),
            normalize_quirks(&first),
            "{name:?}: emfsvg-profile output diverges beyond the declared quirk set"
        );
        let second_document = vectory::decode(&first)
            .unwrap_or_else(|e| panic!("{name:?}: re-decode failed: {e}"));
        let second = vectory::serialize(&second_document, vectory::Profile::EmfsvgSvgBuilder);
        assert_eq!(first, second, "{name:?}: not byte-stable across two round-trips");
    }
}

#[test]
fn emfsvg_declaration_is_preserved_verbatim() {
    // The renderer emits the literal double-space declaration; the model
    // carries it verbatim and the EmfsvgSvgBuilder profile emits it back.
    let source = r#"<?xml version="1.0"  encoding="UTF-8" standalone="no"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="10" height="10">
<g transform="translate(0.0000, 0.00 00)">
</g>
</svg>
"#;
    let out = vectory::roundtrip(source, vectory::Profile::EmfsvgSvgBuilder).expect("round-trip");
    assert!(out.starts_with("<?xml version=\"1.0\"  encoding=\"UTF-8\" standalone=\"no\"?>"));
    assert!(out.contains("translate(0.0000, 0.00 00)"));
    assert_eq!(out, source);
}

#[test]
fn emfsvg_attribute_order_is_pure_model_order() {
    // Renderer outputs carry xmlns mid-list; the EmfsvgSvgBuilder profile
    // must NOT hoist declarations (that is the RubyXml profile's rule).
    let source = r##"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="473.0000" height="528.0000">
</svg>
"##;
    let out = vectory::roundtrip(source, vectory::Profile::EmfsvgSvgBuilder).expect("round-trip");
    assert_eq!(out, source);
    let ruby = vectory::roundtrip(source, vectory::Profile::RubyXml).expect("round-trip");
    assert!(
        ruby.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "RubyXml profile hoists xmlns declarations: {ruby}"
    );
}

#[test]
fn emfsvg_cdata_and_text_roundtrip() {
    let source = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\">\n",
        "<text x=\"1\" y=\"2\"><![CDATA[x<y & raw]]></text>\n",
        "</svg>\n"
    );
    let out = vectory::roundtrip(source, vectory::Profile::EmfsvgSvgBuilder).expect("round-trip");
    assert_eq!(out, source);
}
