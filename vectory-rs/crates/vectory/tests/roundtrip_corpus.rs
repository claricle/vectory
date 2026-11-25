//! Corpus gate: decode → serialize (RubyXml profile) must be byte-identical
//! to the Ruby producer's bytes (`Leptris::XML.parse(content).root.to_xml`,
//! goldens under `vectory-rs/testdata/golden`, regenerable via
//! `vectory-rs/scripts/gen_ruby_golden.rb`), and byte-stable under a second
//! round-trip. The two corpus files leptris parses but that have no root
//! element must fail decode with a named error (vectory raises ParsingError
//! on them).

mod common;

use common::{corpus_files, decode_or_err, golden_path, repo_spec_dir};

#[test]
fn corpus_roundtrip_matches_ruby_producer_bytes() {
    let files = corpus_files();
    let mut exact = 0;
    let mut errored = 0;
    for (rel, source) in &files {
        let golden_path = golden_path(rel);
        match std::fs::read_to_string(&golden_path) {
            Ok(golden) => {
                let first = vectory::roundtrip(source, vectory::Profile::RubyXml)
                    .unwrap_or_else(|e| panic!("{rel:?}: decode failed: {e}"));
                assert_eq!(first, golden, "{rel:?}: round-trip diverges from Ruby bytes");
                let second = vectory::roundtrip(&first, vectory::Profile::RubyXml)
                    .unwrap_or_else(|e| panic!("{rel:?}: decode of own output failed: {e}"));
                assert_eq!(second, first, "{rel:?}: round-trip is not byte-stable");
                exact += 1;
            }
            Err(_) => {
                decode_or_err(source, rel);
                errored += 1;
            }
        }
    }
    assert_eq!(exact, 52, "expected 52 byte-exact corpus files");
    assert_eq!(errored, 2, "expected exactly 2 named-error corpus files");
    assert_eq!(files.len(), 54, "corpus size drifted");
}

#[test]
fn corpus_files_without_root_error_with_named_errors() {
    // odf.svg carries unquoted attribute values (invalid XML that leptris
    // recovers into a rootless document; vectory raises ParsingError).
    let source =
        std::fs::read_to_string(repo_spec_dir().join("examples/odf.svg")).expect("corpus file");
    match vectory::decode(&source) {
        Ok(_) => panic!("odf.svg must not decode"),
        Err(e) => assert!(e.to_string().contains("invalid XML"), "named error: {e}"),
    }
    // eps_but_svg_extension.svg is PostScript despite its extension.
    let source = std::fs::read_to_string(repo_spec_dir().join("examples/eps_but_svg_extension.svg"))
        .expect("corpus file");
    assert!(vectory::decode(&source).is_err(), "postscript file must not decode");
}
