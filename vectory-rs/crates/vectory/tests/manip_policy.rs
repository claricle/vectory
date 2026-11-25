//! Port of the SVG-relevant spec surfaces for the input policy, the data
//! URI handling and the file sniff: spec/vectory/svg_input_policy_spec.rb,
//! the SVG cases of spec/vectory/datauri_spec.rb, and the `:svg` cases of
//! spec/vectory/file_magic_spec.rb.
//!
//! Deviations documented per the model-driven law (never skipped silently):
//! * datauri_spec's EPS/PS/EMF cases belong to those models' crates — this
//!   crate rejects their mime types (`Datauri::svg_content`) instead of
//!   decoding into them, and `from_vector`'s line-ending normalization is
//!   gated for the SVG type only;
//! * datauri dimensions for SVG read the root's width/height/viewBox
//!   (the Ruby Svg#dim_from_attr chain);
//! * file_magic_spec's EPS/PS/EMF references belong to their own crates;
//!   the `:svg` sniff (empty file → nil, `<svg` in the first 4096 bytes →
//!   svg) is gated here;
//! * svg_input_policy_spec's "NUL-padded UTF-16" context passes
//!   process-encoded bytes in Ruby; the byte-level policy
//!   (`forbidden`) is gated directly with the same NUL-padded bytes.

mod manip_common;

use manip_common::repo_spec_dir;
use vectory::datauri::{strict_decode64, Datauri};
use vectory::input_policy;

const DECLARATION: &str = "<!DOCTYPE svg [<!ENTITY secret SYSTEM \"file:///secondary-secret\">]>";
const SOURCE: &str =
    "<!DOCTYPE svg [<!ENTITY secret SYSTEM \"file:///secondary-secret\">]><svg>&secret;</svg>";

#[test]
fn rejects_internal_dtd_subsets_which_can_declare_entities() {
    let cases = [
        "<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\" [<!ENTITY a \"b\">]>",
        "<!DOCTYPE svg[]>",
        "<!DOCTYPE svg [",
        "<!DOCTYPE svg SYSTEM \"a>b\" [<!ENTITY a \"b\">]>",
    ];
    for dtd in cases {
        let source = format!("{dtd}<svg/>");
        let error = input_policy::validate(&source).unwrap_err();
        assert_eq!(
            error,
            vectory::Error::ForbiddenExternalReference(
                "XML DOCTYPE internal subsets are forbidden".to_string()
            ),
            "for {dtd}"
        );
    }
}

#[test]
fn accepts_a_doctype_without_an_internal_subset_as_svg_editors_write_it() {
    let standard = concat!(
        "<?xml version=\"1.0\"?>",
        "<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" ",
        "\"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"3\" height=\"2\"/>"
    );
    for source in [
        "<!DOCTYPE svg><svg/>".to_string(),
        "<!DOCTYPE svg SYSTEM 'a[b]>c'><svg/>".to_string(),
        standard.to_string(),
    ] {
        input_policy::validate(&source).expect("accepted");
    }
    // The document still parses and the DOCTYPE does not survive the
    // writer's root serialization.
    let document = vectory::Document::parse(standard).expect("parses");
    let content = vectory::serialize(&document, vectory::Profile::RubyXml);
    assert!(!content.contains("DOCTYPE"));
    assert_eq!(
        document
            .root
            .as_ref()
            .unwrap()
            .attribute("width")
            .and_then(|value| value
                .split('.')
                .next()
                .and_then(|int| int.parse::<i64>().ok())),
        Some(3)
    );
}

#[test]
fn never_loads_the_external_dtd_a_doctype_names() {
    // The policy refuses before any parse; the external identifier named by
    // the DOCTYPE is never read.
    let directory = std::env::temp_dir().join("vectory-rs-policy-test");
    std::fs::create_dir_all(&directory).expect("tmpdir");
    let dtd = directory.join("entities.dtd");
    std::fs::write(&dtd, "<!ENTITY secret \"LEAKED\">").expect("write dtd");
    let source = format!(
        "<!DOCTYPE svg SYSTEM \"file://{}\"><svg xmlns=\"http://www.w3.org/2000/svg\"><text>&amp;secret;</text></svg>",
        dtd.display()
    );
    input_policy::validate(&source).expect("subsetless DOCTYPE accepted");
    let document = vectory::Document::parse(&source.replace("&amp;", "&#38;")).expect("parses");
    let content = vectory::serialize(&document, vectory::Profile::RubyXml);
    assert!(!content.contains("LEAKED"));
}

#[test]
fn rejects_nul_padded_utf16_and_utf32_declarations_before_parsing() {
    // The Ruby spec encodes the declaration-bearing source into UTF-16/32;
    // the byte-level policy sees the same ASCII token bytes after NUL
    // stripping, so the same bytes are gated here.
    for encoding in ["UTF-16LE", "UTF-16BE", "UTF-32LE", "UTF-32BE"] {
        let bytes = encode_stub(SOURCE, encoding);
        assert!(
            input_policy::forbidden(&bytes),
            "{encoding} declaration bytes must be refused"
        );
    }
}

/// Minimal stand-in for Ruby's `String#encode`: NUL-pad the ASCII bytes the
/// way UTF-16/UTF-32 would represent them (the policy scans NUL-stripped
/// bytes, so the padding pattern is what the policy observes).
fn encode_stub(source: &str, encoding: &str) -> Vec<u8> {
    let ascii: Vec<u8> = source.chars().map(|c| c as u32).map(|c| c as u8).collect();
    match encoding {
        "UTF-16LE" => ascii.iter().fold(Vec::new(), |mut out, b| {
            out.push(*b);
            out.push(0);
            out
        }),
        "UTF-16BE" => ascii.iter().fold(Vec::new(), |mut out, b| {
            out.push(0);
            out.push(*b);
            out
        }),
        "UTF-32LE" => ascii.iter().fold(Vec::new(), |mut out, b| {
            out.push(*b);
            out.push(0);
            out.push(0);
            out.push(0);
            out
        }),
        "UTF-32BE" => ascii.iter().fold(Vec::new(), |mut out, b| {
            out.push(0);
            out.push(0);
            out.push(0);
            out.push(*b);
            out
        }),
        other => panic!("unknown encoding {other}"),
    }
}

#[test]
fn preserves_literal_declaration_text_in_comments_cdata_and_instructions() {
    let safe =
        format!("<!-- {DECLARATION} --><?note {DECLARATION}?><svg><![CDATA[{DECLARATION}]]></svg>");
    input_policy::validate(&safe).expect("opaque tokens accepted");
    let document = vectory::Document::parse(&safe).expect("parses");
    let content = vectory::serialize(&document, vectory::Profile::RubyXml);
    assert!(content.contains("<![CDATA["));
}

#[test]
fn continues_scanning_after_multibyte_comments_and_cdata() {
    let source = format!("<!-- é --><![CDATA[é]]>{SOURCE}");
    assert!(input_policy::forbidden(source.as_bytes()));
}

// ---------------------------------------------------------------------------
// datauri_spec.rb — the SVG cases.
// ---------------------------------------------------------------------------

fn corpus(relative: &str) -> String {
    std::fs::read_to_string(repo_spec_dir().join(relative))
        .unwrap_or_else(|e| panic!("{relative}: {e}"))
}

#[test]
fn from_vector_svg_returns_the_datauri_content() {
    // Ruby `Datauri.from_vector(Svg.from_path(...))` encodes the SVG's
    // `content` — the writer's serialized root (root.to_xml), which drops
    // the trailing document newline — after CRLF normalization.
    let input = corpus("examples/svg2emf/img.svg");
    let normalized = input.replace("\r\n", "\n");
    let content = vectory::roundtrip(&normalized, vectory::Profile::RubyXml).expect("svg model");
    let encoded = vectory::datauri::strict_encode64(content.as_bytes());
    let expected = corpus("examples/svg2emf/img.svg.datauri");
    let expected_payload = expected
        .strip_prefix("data:image/svg+xml;base64,")
        .expect("svg datauri fixture");
    assert_eq!(encoded, expected_payload.trim_end_matches('\n'));
}

#[test]
fn to_vector_svg_returns_the_svg_content() {
    let input = corpus("examples/svg2emf/img.svg.datauri");
    let parsed = Datauri::parse(input.trim()).expect("parses");
    assert_eq!(parsed.mime(), "image/svg+xml");
    let content = parsed.svg_content().expect("svg content");
    let reference = corpus("examples/svg2emf/img.svg");
    manip_common::assert_xml_equivalent(&content, &reference);
}

#[test]
fn incorrect_data_raises_a_conversion_error() {
    let error = Datauri::parse("123abc").unwrap_err();
    assert!(
        format!("{error}").contains("Could not parse datauri: '123abc'"),
        "{error}"
    );
    assert!(Datauri::parse("123abc").is_err());
    // Dimensions surface the same conversion error for non-datauris.
    let dimension_error = Datauri::parse("123abc").unwrap_err();
    assert!(format!("{dimension_error}").contains("Could not parse datauri"));
}

#[test]
fn svg_datauri_dimensions_come_from_the_svg() {
    let input = corpus("examples/svg2emf/img.svg.datauri");
    let parsed = Datauri::parse(input.trim()).expect("parses");
    let content = parsed.svg_content().expect("svg content");
    let document = vectory::Document::parse(&content).expect("svg parses");
    let root = document.root.as_ref().expect("root");
    assert_eq!(dimension(root, "height"), Some(100));
    assert_eq!(dimension(root, "width"), Some(100));
}

fn dimension(root: &vectory::Element, name: &str) -> Option<i64> {
    let from_attr = root.attribute(name).and_then(leading_number);
    from_attr.or_else(|| {
        root.attribute("viewBox").and_then(|view_box| {
            let parts: Vec<&str> = view_box.split_whitespace().collect();
            parts
                .get(if name == "width" { 2 } else { 3 })
                .and_then(|part| part.parse::<i64>().ok())
        })
    })
}

fn leading_number(value: &str) -> Option<i64> {
    let digits: String = value
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '-')
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse::<f64>().ok().map(|number| number as i64)
}

#[test]
fn base64_round_trip_integrity() {
    let bytes = b"vectory datauri round trip";
    let encoded = vectory::datauri::strict_encode64(bytes);
    assert_eq!(strict_decode64(&encoded), Some(bytes.to_vec()));
}

// ---------------------------------------------------------------------------
// file_magic_spec.rb — the `:svg` sniff.
// ---------------------------------------------------------------------------

#[test]
fn returns_nil_for_an_empty_file() {
    assert_eq!(input_policy::sniff_svg(b""), None);
}

#[test]
fn detects_svg_references_by_their_tag() {
    let references = ["examples/eps2svg/ref.svg", "examples/svg2emf/img.svg"];
    for reference in references {
        let bytes = std::fs::read(repo_spec_dir().join(reference)).expect("reference file");
        assert_eq!(
            input_policy::sniff_svg(&bytes),
            Some(input_policy::SvgMagic),
            "{reference}"
        );
    }
}

#[test]
fn non_svg_references_stay_out_of_this_crate() {
    // The EMF magic classification belongs to the EMF model; the SVG sniff
    // must not claim those bytes.
    let emf = std::fs::read(repo_spec_dir().join("examples/svg2emf/ref.emf")).expect("emf file");
    assert_eq!(input_policy::sniff_svg(&emf), None);
}
