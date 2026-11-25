//! The derived serializer rules (see the crate docs), each pinned by a test.
//! Every expectation here was probe-verified against the Ruby producers
//! (leptris/libxml2 for the RubyXml profile; SvgBuilder source for the
//! EmfsvgSvgBuilder profile).

use vectory::{decode, roundtrip, serialize, Document, Profile};

fn ruby(content: &str) -> String {
    roundtrip(content, Profile::RubyXml).expect("RubyXml round-trip")
}

fn emf(content: &str) -> String {
    roundtrip(content, Profile::EmfsvgSvgBuilder).expect("EmfsvgSvgBuilder round-trip")
}

// R2/R3: double-quoted attributes; self-closing empty form (both profiles).
#[test]
fn empty_elements_serialize_self_closing() {
    assert_eq!(ruby("<r><a></a></r>"), "<r><a/></r>");
    assert_eq!(ruby("<r><a/></r>"), "<r><a/></r>");
    assert_eq!(emf("<r><a></a></r>"), "<r><a/></r>");
    assert_eq!(emf("<r><a/></r>"), "<r><a/></r>");
}

// R4: no invented whitespace; whitespace-only text survives.
#[test]
fn whitespace_only_text_survives() {
    assert_eq!(ruby("<r>\n  <a/>\n</r>"), "<r>\n  <a/>\n</r>");
}

// R11/R21: text escaping.
#[test]
fn text_escaping_matches_producer() {
    assert_eq!(ruby("<r><t>a &gt; b &amp; c &lt; d</t></r>"), "<r><t>a &gt; b &amp; c &lt; d</t></r>");
    assert_eq!(ruby("<r><t>a > b &amp; c</t></r>"), "<r><t>a &gt; b &amp; c</t></r>");
    assert_eq!(ruby("<r><t>it's a \"test\"</t></r>"), "<r><t>it's a \"test\"</t></r>");
    // R11: CR re-escapes as a decimal character reference.
    assert_eq!(ruby("<r><t>a&#xD;b</t></r>"), "<r><t>a&#13;b</t></r>");
    // R13: literal line ends normalize to LF at decode.
    assert_eq!(ruby("<r><t>a\r\nb</t></r>"), "<r><t>a\nb</t></r>");
    assert_eq!(ruby("<r><t>a\rb</t></r>"), "<r><t>a\nb</t></r>");
    assert_eq!(ruby("<r><t>a\tb</t></r>"), "<r><t>a\tb</t></r>");
}

// R5: CDATA stays a distinct, unmerged, verbatim node.
#[test]
fn cdata_preserved_and_not_merged() {
    assert_eq!(ruby("<r><t><![CDATA[<x>&y]]></t></r>"), "<r><t><![CDATA[<x>&y]]></t></r>");
    assert_eq!(ruby("<r><t>a<![CDATA[b]]>c</t></r>"), "<r><t>a<![CDATA[b]]>c</t></r>");
    assert_eq!(ruby("<r><style><![CDATA[ a > b ]]></style></r>"), "<r><style><![CDATA[ a > b ]]></style></r>");
}

// R6/R7: comments and PIs verbatim.
#[test]
fn comments_and_pis_survive() {
    assert_eq!(ruby("<r><!-- a -- b --></r>"), "<r><!-- a -- b --></r>");
    assert_eq!(ruby("<r><t>a</t><!-- c --><t>b</t></r>"), "<r><t>a</t><!-- c --><t>b</t></r>");
    let pi = "<?xml-stylesheet type=\"text/xsl\" href=\"s.xsl\"?><r/>";
    let doc = decode(pi).expect("parse");
    assert_eq!(
        serialize(&doc, Profile::EmfsvgSvgBuilder),
        pi,
        "EmfsvgSvgBuilder emits prolog PIs verbatim"
    );
}

// R10: RubyXml drops declaration, DOCTYPE, prolog and epilog.
#[test]
fn ruby_profile_emits_root_subtree_only() {
    assert_eq!(ruby("<?xml version=\"1.0\"?><!-- before --><r/><!-- after -->"), "<r/>");
    assert_eq!(
        ruby("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\"><svg/>"),
        "<svg/>"
    );
    assert_eq!(ruby("<?xml-stylesheet type=\"text/xsl\" href=\"s.xsl\"?><r/>"), "<r/>");
}

// R12: attribute-value escaping.
#[test]
fn attribute_escaping_matches_producer() {
    assert_eq!(ruby("<r><a x='say \"hi\"'/></r>"), "<r><a x=\"say &quot;hi&quot;\"/></r>");
    assert_eq!(ruby("<r><a x=\"it's\"/></r>"), "<r><a x=\"it's\"/></r>");
    assert_eq!(ruby("<r><a x=\"a&lt;b\"/></r>"), "<r><a x=\"a&lt;b\"/></r>");
    assert_eq!(ruby("<r><a x=\"a>b\"/></r>"), "<r><a x=\"a&gt;b\"/></r>");
    assert_eq!(ruby("<r><a x=\"&amp;&quot;&lt;&gt;&#13;\"/></r>"), "<r><a x=\"&amp;&quot;&lt;&gt;&#13;\"/></r>");
    // R13: literal TAB/LF normalize to spaces in attribute values, while
    // character references survive and re-escape on output.
    assert_eq!(ruby("<r><a x=\"a\nb\"/></r>"), "<r><a x=\"a b\"/></r>");
    assert_eq!(ruby("<r><a x=\"a\tb\"/></r>"), "<r><a x=\"a b\"/></r>");
    assert_eq!(ruby("<r><a x=\"&#10;\t\"/></r>"), "<r><a x=\"&#10; \"/></r>");
    assert_eq!(ruby("<r><a x=\"a&#10;b\"/></r>"), "<r><a x=\"a&#10;b\"/></r>");
    assert_eq!(ruby("<r><a x=\"a&#9;b\"/></r>"), "<r><a x=\"a&#9;b\"/></r>");
    assert_eq!(ruby("<r><a x=\"a&#xD;b\"/></r>"), "<r><a x=\"a&#13;b\"/></r>");
}

// R1 + RubyXml exception: xmlns declarations hoist ahead of attributes.
#[test]
fn ruby_profile_hoists_namespace_declarations() {
    assert_eq!(
        ruby("<r xmlns=\"urn:root\"><c p:a=\"1\" q:b=\"2\" xmlns:p=\"urn:b\" xmlns:q=\"urn:q\" z=\"3\"/></r>"),
        "<r xmlns=\"urn:root\"><c xmlns:p=\"urn:b\" xmlns:q=\"urn:q\" p:a=\"1\" q:b=\"2\" z=\"3\"/></r>"
    );
    // The hoist happens at every element, in declaration order.
    assert_eq!(
        ruby("<svg xmlns=\"urn:a\" version=\"1.1\" id=\"Layer_1\" xmlns:xlink=\"urn:xlink\"/>"),
        "<svg xmlns=\"urn:a\" xmlns:xlink=\"urn:xlink\" version=\"1.1\" id=\"Layer_1\"/>"
    );
}

// R8: names emit lexically; redeclarations and undeclarations survive.
#[test]
fn namespace_redeclarations_survive_lexically() {
    assert_eq!(
        ruby("<r xmlns:p=\"urn:a\"><c xmlns:p=\"urn:b\" p:a=\"1\"/><c p:a=\"2\"/></r>"),
        "<r xmlns:p=\"urn:a\"><c xmlns:p=\"urn:b\" p:a=\"1\"/><c p:a=\"2\"/></r>"
    );
    assert_eq!(
        ruby("<r xmlns:p=\"urn:a\"><c xmlns:p=\"\"/></r>"),
        "<r xmlns:p=\"urn:a\"><c xmlns:p=\"\"/></r>"
    );
}

// Entity references decode into the surrounding text and re-escape.
#[test]
fn entity_references_resolve_into_text() {
    assert_eq!(ruby("<r><t>a&#38;b&#60;</t></r>"), "<r><t>a&amp;b&lt;</t></r>");
    assert_eq!(ruby("<r><t>a &gt; b</t></r>"), "<r><t>a &gt; b</t></r>");
}

// Namespace resolution mirrors svg_namespace_binding.rb.
#[test]
fn namespace_resolution_semantics() {
    let doc = decode("<r xmlns=\"urn:root\" xmlns:p=\"urn:p\"><c xmlns:p=\"urn:inner\"><p:a p:b=\"1\" q:c=\"2\"/></c><d/></r>")
        .expect("parse");
    let root = doc.root.as_ref().expect("root");
    assert_eq!(root.namespace_uri.as_deref(), Some("urn:root"));
    let c = root.child_elements().next().expect("c");
    assert_eq!(c.namespace_uri.as_deref(), Some("urn:root"));
    // Shadowing: p redeclared on the inner element applies to p:a below it.
    let p_a = c.child_elements().next().expect("p:a");
    assert_eq!(p_a.namespace_uri.as_deref(), Some("urn:inner"));
    // Prefixed attribute resolves; unprefixed attribute has NO namespace;
    // unbound prefix stays unbound.
    assert_eq!(p_a.attributes[0].namespace_uri.as_deref(), Some("urn:inner"));
    assert_eq!(p_a.attributes[1].namespace_uri, None);
    // xml: prefix binds to the XML namespace.
    let doc = decode("<r xml:lang=\"en\"/>").expect("parse");
    let root = doc.root.as_ref().expect("root");
    assert_eq!(root.attributes[0].namespace_uri.as_deref(), Some(vectory::namespace::XML_NAMESPACE_URI));
}

#[test]
fn unbound_prefixes_validate_named() {
    let doc = decode("<r><p:a p:b=\"1\"/></r>").expect("parse is lenient");
    assert!(vectory::namespace::validate_prefixes(&doc).is_err());
    let doc = decode("<r xmlns=\"urn:r\"><p:a/></r>").expect("parse");
    assert!(vectory::namespace::validate_prefixes(&doc).is_err());
    let doc = decode("<r xmlns:p=\"urn:p\"><p:a/></r>").expect("parse");
    assert!(vectory::namespace::validate_prefixes(&doc).is_ok());
}

// Malformed input raises named errors.
#[test]
fn malformed_input_errors() {
    assert!(matches!(decode("<r><a></r>"), Err(vectory::Error::Xml(_))));
    assert!(matches!(decode("plain text"), Err(vectory::Error::NoRoot)));
    assert!(matches!(decode("<r/><extra/>"), Err(vectory::Error::Xml(_))));
    assert!(matches!(decode("<r/>junk"), Err(vectory::Error::Xml(_))));
    assert!(matches!(decode("<r>&nbsp;</r>"), Err(vectory::Error::Xml(_))));
}

// The model document API round-trips through Document::parse/serialize.
#[test]
fn document_api_roundtrip() {
    let doc = Document::parse("<svg xmlns=\"urn:a\"><g><text>x</text></g></svg>").expect("parse");
    assert_eq!(
        doc.serialize(Profile::RubyXml),
        "<svg xmlns=\"urn:a\"><g><text>x</text></g></svg>"
    );
    assert_eq!(doc.elements().count(), 3);
}
