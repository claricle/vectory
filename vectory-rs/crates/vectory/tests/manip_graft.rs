//! Port of spec/vectory/svg_subtree_graft_spec.rb — envelope preservation,
//! namespace sealing, and the stable error contract.
//!
//! Ruby-process-bound deviations (documented, not skipped): the
//! ISO-8859-1/UTF-16 host contexts re-encode raw bytes in the Ruby process
//! (`String#encode`); the Rust boundary is UTF-8 by contract, so the
//! declaration-rewrite semantics are gated with UTF-8-declared content.

mod manip_common;

use manip_common::{assert_xml_equivalent, transform_fixture};
use vectory::graft::replace_root_content;

#[test]
fn grafted_content_matches_the_expected_documents() {
    for case in [
        "changed",
        "metadata",
        "mixed",
        "namespaces",
        "empty",
        "repeat",
    ] {
        let host = transform_fixture(&format!("subtree_graft/{case}/host.xml"));
        let processed = transform_fixture(&format!("subtree_graft/{case}/processed.xml"));
        let expected = transform_fixture(&format!("subtree_graft/{case}/expected.xml"));
        let result = replace_root_content(&host, &processed).expect("graft succeeds");
        assert_xml_equivalent(&result, &expected);
        // Idempotent when applied repeatedly.
        let repeated = replace_root_content(&result, &processed).expect("repeat succeeds");
        assert_xml_equivalent(&repeated, &result);
    }
}

#[test]
fn host_metadata_retains_root_attributes_and_envelope() {
    let host = transform_fixture("subtree_graft/metadata/host.xml");
    let processed = transform_fixture("subtree_graft/metadata/processed.xml");
    let result = replace_root_content(&host, &processed).expect("graft succeeds");
    assert!(result.contains("id=\"host\""));
    assert!(result.contains("h:flag=\"yes\""));
    assert!(result.contains("<!--before--><?host keep?><!DOCTYPE article>"));
    assert!(result.contains("</article><!--after--><?tail keep?>"));
    assert!(!result.contains("id=\"discard\""));
    assert!(!result.contains("<old"));
}

#[test]
fn mixed_content_preserves_the_ordered_node_kinds_and_values() {
    let host = transform_fixture("subtree_graft/mixed/host.xml");
    let processed = transform_fixture("subtree_graft/mixed/processed.xml");
    let result = replace_root_content(&host, &processed).expect("graft succeeds");
    assert!(result.contains("before<!--middle--><![CDATA[a<b & c]]><?graft keep?>"));
    assert!(result.contains("after"));
}

#[test]
fn retains_an_envelope_containing_markup_like_text_and_whitespace() {
    let prefix = "<?xml version=\"1.0\"?>\n<!--é--><!DOCTYPE host [<!ELEMENT host ANY>]>\n";
    let suffix = " \n<!-- </host> --><?tail </host>?>\n";
    let content = format!("{prefix}<host><!--old--><old/></host>{suffix}");
    let output = replace_root_content(&content, "<svg/>").expect("graft succeeds");
    assert_eq!(output, format!("{prefix}<host/>{suffix}"));
}

#[test]
fn conflicting_namespaces_retain_host_metadata_and_grafted_expanded_names() {
    let host = transform_fixture("subtree_graft/namespaces/host.xml");
    let processed = transform_fixture("subtree_graft/namespaces/processed.xml");
    let result = replace_root_content(&host, &processed).expect("graft succeeds");
    let document = vectory::Document::parse(&result).expect("reparses");
    let root = document.root.as_ref().expect("root");
    assert_eq!(root.name, "h:host");
    // The grafted tree keeps its expanded names after reparsing: the g
    // element is urn:svg, shape/plain/other carry urn:processed-prefix,
    // and bare/plain/child are unnamespaced.
    let mut collected: Vec<&vectory::Element> = Vec::new();
    vectory::mutation::elements(root, &mut collected);
    let names: Vec<(String, Option<String>)> = collected
        .iter()
        .map(|element| {
            (
                element.local_name().to_string(),
                element.namespace_uri.clone(),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("host".to_string(), Some("urn:host".to_string())),
            ("g".to_string(), Some("urn:svg".to_string())),
            (
                "shape".to_string(),
                Some("urn:processed-prefix".to_string())
            ),
            ("plain".to_string(), None),
            ("child".to_string(), None),
            (
                "other".to_string(),
                Some("urn:processed-prefix".to_string())
            ),
            ("bare".to_string(), None),
        ]
    );
    // p:flag on g stays in the processed prefix; xlink:href stays xlink.
    // (Nokogiri `attribute_nodes` excludes namespace definitions.)
    let g = &collected[1];
    let g_attributes: Vec<&vectory::Attribute> = g
        .attributes
        .iter()
        .filter(|attribute| !attribute.is_namespace_declaration())
        .collect();
    assert_eq!(
        g_attributes[0].namespace_uri,
        Some("urn:processed-prefix".to_string())
    );
    let shape = &collected[2];
    let shape_attributes: Vec<&vectory::Attribute> = shape
        .attributes
        .iter()
        .filter(|attribute| !attribute.is_namespace_declaration())
        .collect();
    assert_eq!(
        shape_attributes[0].namespace_uri,
        Some("http://www.w3.org/1999/xlink".to_string())
    );
}

#[test]
fn retains_every_attribute_configuration_through_the_graft() {
    for attributes in [
        "a=\"1\" xlink:href=\"#g\" c=\"3\"",
        "xlink:href=\"#g\" fill=\"x\"",
        "fill=\"x\" xlink:href=\"#g\"",
    ] {
        for nested in [false, true] {
            let rect = format!("<rect {attributes}/>");
            let content = if nested {
                format!("<g>{rect}</g>")
            } else {
                rect.clone()
            };
            let source = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">{content}</svg>"
            );
            let output =
                replace_root_content("<svg xmlns=\"http://www.w3.org/2000/svg\"/>", &source)
                    .expect("graft succeeds");
            let expected = vectory::Document::parse(&source).expect("parses");
            let actual = vectory::Document::parse(&output).expect("parses");
            let mut expected_attrs = Vec::new();
            let mut actual_attrs = Vec::new();
            collect_rect_attributes(expected.root.as_ref().unwrap(), &mut expected_attrs);
            collect_rect_attributes(actual.root.as_ref().unwrap(), &mut actual_attrs);
            assert_eq!(
                actual_attrs, expected_attrs,
                "attributes lost for {attributes} nested={nested}: {output}"
            );
        }
    }
}

fn collect_rect_attributes(element: &vectory::Element, out: &mut Vec<(String, String, String)>) {
    if element.local_name() == "rect" {
        // Nokogiri `attribute_nodes` excludes namespace definitions.
        for attribute in &element.attributes {
            if attribute.is_namespace_declaration() {
                continue;
            }
            out.push((
                attribute.namespace_uri.clone().unwrap_or_default(),
                attribute.name.clone(),
                attribute.value.clone(),
            ));
        }
    }
    for child in &element.children {
        if let vectory::Node::Element(child_element) = child {
            collect_rect_attributes(child_element, out);
        }
    }
}

#[test]
fn rejects_malformed_host_xml_with_the_stable_prefix() {
    let error = replace_root_content("<host>", "<svg/>").unwrap_err();
    assert!(
        matches!(error, vectory::Error::Parsing(detail) if detail.starts_with("invalid host_content XML:"))
    );
}

#[test]
fn rejects_malformed_processed_xml_with_the_stable_prefix() {
    let error = replace_root_content("<host/>", "<svg>").unwrap_err();
    assert!(
        matches!(error, vectory::Error::Parsing(detail) if detail.starts_with("invalid processed_content XML:"))
    );
}

#[test]
fn rejects_a_rootless_host_with_the_exact_message() {
    let error = replace_root_content("<!--host-->", "<svg/>").unwrap_err();
    assert_eq!(
        error,
        vectory::Error::Parsing("host_content has no document root".to_string())
    );
}

#[test]
fn rejects_rootless_processed_content_with_the_exact_message() {
    let error = replace_root_content("<host/>", "<?xml version='1.0'?>").unwrap_err();
    assert_eq!(
        error,
        vectory::Error::Parsing("processed_content has no document root".to_string())
    );
}

#[test]
fn does_not_classify_malformed_rootless_content_as_a_missing_root() {
    let error = replace_root_content("<!--unterminated", "<svg/>").unwrap_err();
    assert!(
        matches!(error, vectory::Error::Parsing(detail) if detail.starts_with("invalid host_content XML:"))
    );
}

#[test]
fn keeps_the_utf8_declared_envelope_verbatim() {
    // The encoding-normalization semantics (Ruby re-encodes ISO-8859-1 and
    // UTF-16 hosts to UTF-8) are gated here with a UTF-8-declared host:
    // metadata, ordered Unicode content, and the envelope all survive.
    let host =
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--café--><host label=\"été\"/><!--fin été-->";
    let processed = "<svg><text>été 日本語</text><!--日本語--><![CDATA[日本語]]></svg>";
    let result = replace_root_content(host, processed).expect("graft succeeds");
    assert!(
        result.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--café-->"),
        "{result}"
    );
    assert!(result.ends_with("<!--fin été-->"));
    let document = vectory::Document::parse(&result).expect("parses");
    let root = document.root.as_ref().expect("root");
    assert_eq!(root.attribute("label"), Some("été"));
}

#[test]
fn does_not_mistake_an_xml_stylesheet_pi_for_the_declaration() {
    let prefix = "<?xml-stylesheet type=\"text/css\" href=\"café.css\" encoding=\"ISO-8859-1\"?>";
    let output = replace_root_content(&format!("{prefix}<host label='été'/>"), "<svg/>")
        .expect("graft succeeds");
    assert!(output.starts_with(prefix));
    let document = vectory::Document::parse(&output).expect("parses");
    assert_eq!(
        document.root.as_ref().expect("root").attribute("label"),
        Some("été")
    );
}
