//! Port of spec/vectory/svg_id_disambiguator_spec.rb and the class-suffixer
//! parts of spec/vectory/svg_class_suffixer_spec.rb — the exact renaming
//! rules.

mod manip_common;

use std::collections::HashMap;

use manip_common::{find_first, style_children_xml, text_of, transform_fixture};
use vectory::id_disambiguator;

fn svg_a() -> String {
    concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">\n",
        "  <defs>\n",
        "    <linearGradient id=\"grad\"><stop stop-color=\"red\"/></linearGradient>\n",
        "    <clipPath id=\"clip\"><rect width=\"1\" height=\"1\"/></clipPath>\n",
        "  </defs>\n",
        "  <style>#grad { fill: url(#grad) }</style>\n",
        "  <use xlink:href=\"#grad\" href=\"#grad\"/>\n",
        "  <g id=\"shape\"><path clip-path=\"url(#clip)\" style=\"fill: url(#grad)\"/></g>\n",
        "</svg>\n"
    )
    .to_string()
}

fn svg_b() -> String {
    concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\">\n",
        "  <defs>\n",
        "    <linearGradient id=\"grad\"><stop stop-color=\"blue\"/></linearGradient>\n",
        "    <clipPath id=\"clip\"><rect width=\"2\" height=\"2\"/></clipPath>\n",
        "  </defs>\n",
        "  <g id=\"shape\"><circle id=\"dot\" r=\"1\"/></g>\n",
        "  <use xlink:href=\"#grad\"/>\n",
        "  <path clip-path=\"url(#clip)\"/>\n",
        "  <path marker-mid=\"url(#marker-only)\"/>\n",
        "</svg>\n"
    )
    .to_string()
}

fn dupes() -> HashMap<String, usize> {
    let a = svg_a();
    let b = svg_b();
    id_disambiguator::duplicate_ids(&[&a, &b]).expect("duplicate ids count")
}

#[test]
fn counts_ids_that_occur_in_more_than_one_document() {
    let dupes = dupes();
    let mut expected = HashMap::new();
    expected.insert("shape".to_string(), 2);
    expected.insert("grad".to_string(), 2);
    expected.insert("clip".to_string(), 2);
    assert_eq!(dupes, expected);
}

#[test]
fn ignores_single_occurrence_ids() {
    let dupes = dupes();
    assert!(!dupes.contains_key("dot"));
    assert!(!dupes.contains_key("marker-only"));
}

#[test]
fn suffixes_the_colliding_id_attributes() {
    let dupes = dupes();
    let a = svg_a();
    let b = svg_b();
    let out_a = id_disambiguator::rewrite(&a, "_inject_0", &dupes).unwrap();
    let out_b = id_disambiguator::rewrite(&b, "_inject_1", &dupes).unwrap();
    assert!(out_a.contains("id=\"shape_inject_0\""), "{out_a}");
    assert!(out_b.contains("id=\"shape_inject_1\""), "{out_b}");
}

#[test]
fn leaves_unique_ids_untouched() {
    let dupes = dupes();
    let a = svg_a();
    let b = svg_b();
    let out_a = id_disambiguator::rewrite(&a, "_inject_0", &dupes).unwrap();
    let out_b = id_disambiguator::rewrite(&b, "_inject_1", &dupes).unwrap();
    assert!(out_a.contains("id=\"grad_inject_0\""));
    assert!(out_a.contains("id=\"clip_inject_0\""));
    assert!(out_b.contains("id=\"dot\""));
}

#[test]
fn rewrites_href_and_xlink_fragment_references() {
    let dupes = dupes();
    let a = svg_a();
    let out_a = id_disambiguator::rewrite(&a, "_inject_0", &dupes).unwrap();
    assert!(out_a.contains("xlink:href=\"#grad_inject_0\""), "{out_a}");
    assert!(out_a.contains("href=\"#grad_inject_0\""));
}

#[test]
fn rewrites_url_references_in_style_blocks_and_presentation_attributes() {
    let dupes = dupes();
    let a = svg_a();
    let out_a = id_disambiguator::rewrite(&a, "_inject_0", &dupes).unwrap();
    assert!(out_a.contains("fill: url(#grad_inject_0)"), "{out_a}");
    assert!(out_a.contains("clip-path=\"url(#clip_inject_0)\""));
    assert!(out_a.contains("style=\"fill: url(#grad_inject_0)\""));
}

#[test]
fn keeps_non_colliding_references_untouched() {
    let dupes = dupes();
    let b = svg_b();
    let out_b = id_disambiguator::rewrite(&b, "_inject_1", &dupes).unwrap();
    assert!(
        out_b.contains("marker-mid=\"url(#marker-only)\""),
        "{out_b}"
    );
}

#[test]
fn keeps_the_document_parseable_and_all_elements_present() {
    let dupes = dupes();
    let a = svg_a();
    let out_a = id_disambiguator::rewrite(&a, "_inject_0", &dupes).unwrap();
    let document = vectory::Document::parse(&out_a).expect("reparses");
    let root = document.root.as_ref().expect("root");
    let mut names: Vec<&str> = Vec::new();
    let mut collected: Vec<&vectory::Element> = Vec::new();
    vectory::mutation::elements(root, &mut collected);
    for element in collected {
        names.push(element.local_name());
    }
    for expected in ["linearGradient", "clipPath", "use", "path"] {
        assert!(names.contains(&expected), "missing {expected} in {out_a}");
    }
}

#[test]
fn rewrites_svgs_embedded_in_a_host_document() {
    let dupes = dupes();
    let host = format!("<metanorma>\n{}\n{}\n</metanorma>\n", svg_a(), svg_b());
    let out = id_disambiguator::rewrite(&host, "_inject_2", &dupes).unwrap();
    assert!(out.contains("id=\"shape_inject_2\""), "{out}");
    assert_eq!(out.matches("id=\"dot\"").count(), 1);
}

// ---------------------------------------------------------------------------
// svg_class_suffixer_spec.rb
// ---------------------------------------------------------------------------

use vectory::class_suffixer;

fn style_text(content: &str) -> String {
    let document = vectory::Document::parse(content).expect("parses");
    let root = document.root.as_ref().expect("root");
    let style = find_first(root, "style").expect("style present");
    text_of(style)
}

#[test]
fn suffixes_compound_comma_functional_and_nested_selectors() {
    let input = transform_fixture("class_suffixer/styles-input.svg");
    let expected = transform_fixture("class_suffixer/styles-output.svg");
    let output = class_suffixer::rewrite(&input, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&output, &expected);
    // Declarations and unrelated selector content survive byte-for-byte.
    assert_eq!(style_text(&output), {
        let expected_document = vectory::Document::parse(&expected).unwrap();
        let root = expected_document.root.as_ref().unwrap();
        text_of(find_first(root, "style").unwrap())
    });
    let document = vectory::Document::parse(&output).unwrap();
    let root = document.root.as_ref().unwrap();
    let rect = manip_common::find_all(root, "path")
        .into_iter()
        .find(|path| path.attribute("id") == Some("rect1"))
        .or_else(|| {
            let mut collected: Vec<&vectory::Element> = Vec::new();
            vectory::mutation::elements(root, &mut collected);
            collected
                .into_iter()
                .find(|element| element.attribute("id") == Some("rect1"))
        })
        .expect("rect1 present");
    assert_eq!(rect.attribute("style"), Some("fill: url(#grad1);"));
}

#[test]
fn is_idempotent_for_xml_class_lists_and_nested_css() {
    let input = transform_fixture("class_suffixer/styles-input.svg");
    let output = class_suffixer::rewrite(&input, "_inject_0").expect("rewrites");
    let repeated = class_suffixer::rewrite(&output, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &output);
}

#[test]
fn preserves_xml_child_nodes_while_suffixing_split_tokens() {
    let suffix = "_inject_0";
    let cases = [
        (".a{}<!-- c -->.b{}", ".a_inject_0{}<!-- c -->.b_inject_0{}"),
        (".fo<!-- keep -->o{}", ".fo<!-- keep -->o_inject_0{}"),
        (
            ".a{content:\"<!-- keep -->.b\"}.c{}",
            ".a_inject_0{content:\"<!-- keep -->.b\"}.c_inject_0{}",
        ),
        (
            "<![CDATA[.a]]><!-- keep -->{}<![CDATA[.b{}]]>",
            "<![CDATA[.a_inject_0]]><!-- keep -->{}<![CDATA[.b_inject_0{}]]>",
        ),
    ];
    for (css, expected_css) in cases {
        let source =
            format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><style>{css}</style></svg>");
        let result = class_suffixer::rewrite(&source, suffix).expect("rewrites");
        let document = vectory::Document::parse(&result).expect("reparses");
        let root = document.root.as_ref().unwrap();
        let style = find_first(root, "style").unwrap();
        assert_eq!(style_children_xml(style), expected_css, "for {css}");
        // Idempotent.
        let repeated = class_suffixer::rewrite(&result, suffix).expect("rewrites");
        assert_eq!(repeated, result, "repeat differs for {css}");
    }
}

#[test]
fn suffixes_scope_roots_limits_and_nested_rules_without_changing_declarations() {
    let input = concat!(
        "<svg><style>@media screen { @scope (.box:is(.active)) to (.stop) { ",
        ".foo { content: \".box\" } } }</style><g class=\"box active\"><path class=\"foo stop\"/></g></svg>"
    );
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    let document = vectory::Document::parse(&output).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert_eq!(
        style_text(&output),
        concat!(
            "@media screen { @scope (.box_inject_0:is(.active_inject_0)) to (.stop_inject_0) { ",
            ".foo_inject_0 { content: \".box\" } } }"
        )
    );
    let mut collected: Vec<&vectory::Element> = Vec::new();
    vectory::mutation::elements(root, &mut collected);
    let classes: Vec<String> = collected[1..]
        .iter()
        .filter_map(|element| element.attribute("class"))
        .map(str::to_string)
        .collect();
    assert_eq!(
        classes,
        vec![
            "box_inject_0 active_inject_0".to_string(),
            "foo_inject_0 stop_inject_0".to_string()
        ]
    );
    // Stable on repeat.
    let repeated = class_suffixer::rewrite(&output, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &output);
}

#[test]
fn preserves_escape_spelling_while_suffixing_the_decoded_matching_class() {
    let input = transform_fixture("class_suffixer/escaped-input.svg");
    let expected = transform_fixture("class_suffixer/escaped-output.svg");
    let output = class_suffixer::rewrite(&input, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&output, &expected);
    assert_eq!(
        style_text(&output),
        r".foo\+bar_inject_0, .\66 oo\+bar_inject_0 { fill: red; }"
    );
    let document = vectory::Document::parse(&output).unwrap();
    let root = document.root.as_ref().unwrap();
    let path = manip_common::find_all(root, "path")
        .into_iter()
        .next()
        .expect("path present");
    assert_eq!(path.attribute("class"), Some("foo+bar_inject_0"));
    // Matches the repeat golden without adding another suffix.
    let golden = transform_fixture("class_suffixer/escaped-repeat.svg");
    let repeated = class_suffixer::rewrite(&output, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &golden);
}

#[test]
fn rewrites_nested_selectors_while_leaving_declarations_and_custom_properties_intact() {
    let input = concat!(
        "<svg><style>.a{fill:red; .b{fill:blue}} .c{ &amp; .d {x:y} color:blue; ",
        "@media screen { color:red; > .e:is(.f) {content:\".g\"; .g {x:y}} } ",
        "--theme: { .untouched {x:y} }; --value: .untouched; div.h:hover {x:y}}</style>",
        "<g class=\"a b c d e f g h\"/></svg>"
    );
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    let expected_css = concat!(
        ".a_inject_0{fill:red; .b_inject_0{fill:blue}} .c_inject_0{ & .d_inject_0 {x:y} color:blue; ",
        "@media screen { color:red; > .e_inject_0:is(.f_inject_0) {content:\".g\"; .g_inject_0 {x:y}} } ",
        "--theme: { .untouched {x:y} }; --value: .untouched; div.h_inject_0:hover {x:y}}"
    );
    assert_eq!(style_text(&output), expected_css, "{output}");
    let document = vectory::Document::parse(&output).unwrap();
    let root = document.root.as_ref().unwrap();
    let g = manip_common::find_all(root, "g")
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(
        g.attribute("class"),
        Some("a_inject_0 b_inject_0 c_inject_0 d_inject_0 e_inject_0 f_inject_0 g_inject_0 h_inject_0")
    );
    let repeated = class_suffixer::rewrite(&output, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &output);
}

#[test]
fn keeps_escaped_identifiers_semantically_equal_to_the_suffixed_xml_classes() {
    let input = concat!(
        "<svg><style>.foo\\31, .foo\\000031, .foo\\31 , .foo\\\\31 {x:y}</style>",
        "<g class=\"foo1 foo\\31\"/></svg>"
    );
    let output = class_suffixer::rewrite(input, "b").expect("rewrites");
    assert_eq!(
        style_text(&output),
        r".foo\31 b, .foo\000031b, .foo\31 b, .foo\\31b {x:y}"
    );
    let document = vectory::Document::parse(&output).unwrap();
    let root = document.root.as_ref().unwrap();
    let g = manip_common::find_all(root, "g")
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(g.attribute("class"), Some(r"foo1b foo\31b"));
    let repeated = class_suffixer::rewrite(&output, "b").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &output);
}

#[test]
fn rewrites_all_svg_roots_and_descendants_once_and_preserves_host_content() {
    let input = concat!(
        "<host class=\"host\"><style>.host { fill: red }</style>",
        "<svg class=\"root\"><g class=\"a  b\"><svg><path class=\"a\"/></svg></g></svg>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.a { x: 1 }</style><path class=\"a\"/></svg></host>"
    );
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    let document = vectory::Document::parse(&output).expect("reparses");
    let root = document.root.as_ref().unwrap();
    let mut collected: Vec<&vectory::Element> = Vec::new();
    vectory::mutation::elements(root, &mut collected);
    let classes: Vec<String> = collected
        .iter()
        .filter_map(|element| element.attribute("class"))
        .map(str::to_string)
        .collect();
    assert_eq!(
        classes,
        vec![
            "host".to_string(),
            "root_inject_0".to_string(),
            "a_inject_0  b_inject_0".to_string(),
            "a_inject_0".to_string(),
            "a_inject_0".to_string(),
        ]
    );
    // The host's own style block (outside any svg) is untouched.
    let host_style = manip_common::find_all(root, "style")
        .into_iter()
        .next()
        .expect("host style");
    assert_eq!(text_of(host_style), ".host { fill: red }");
}

#[test]
fn preserves_empty_classes_empty_styles_and_absent_classes() {
    let input = "<svg class=\"\"><style/><g class=\" \"/><path/></svg>";
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&output, input);
}

#[test]
fn rewrites_recognized_selectors_and_preserves_discarded_incomplete_streams() {
    let input = "<svg class=\"foo\"><style>.foo { broken; content: &quot;.foo&quot;; } .unfinished</style></svg>";
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    assert_eq!(
        style_text(&output),
        ".foo_inject_0 { broken; content: \".foo\"; } .unfinished"
    );
}

#[test]
fn keeps_an_unclosed_block_unclosed_while_rewriting_its_known_selector() {
    let output = class_suffixer::rewrite("<svg><style>.foo { fill: red</style></svg>", "_inject_0")
        .expect("rewrites");
    assert_eq!(style_text(&output), ".foo_inject_0 { fill: red");
}

#[test]
fn preserves_unknown_at_rules_without_guessing_at_their_blocks() {
    let output = class_suffixer::rewrite(
        "<svg><style>@custom { value: .foo; }</style></svg>",
        "_inject_0",
    )
    .expect("rewrites");
    assert_eq!(style_text(&output), "@custom { value: .foo; }");
}

#[test]
fn detects_a_suffix_represented_by_css_escapes_from_its_semantic_value() {
    let output = class_suffixer::rewrite(
        r"<svg><style>.foo_inject_\30 { x: 1 }</style></svg>",
        "_inject_0",
    )
    .expect("rewrites");
    assert_eq!(style_text(&output), r".foo_inject_\30 { x: 1 }");
}

#[test]
fn preserves_unicode_identifiers_and_comments_between_class_selector_tokens() {
    let output = class_suffixer::rewrite(
        r#"<svg><style>/*é*/ ./**/café { content: ".café" }</style></svg>"#,
        "_inject_0",
    )
    .expect("rewrites");
    assert_eq!(
        style_text(&output),
        r#"/*é*/ ./**/café_inject_0 { content: ".café" }"#
    );
}

#[test]
fn rejects_invalid_xml() {
    let result = class_suffixer::rewrite("<svg><path></svg>", "_inject_0");
    assert!(matches!(
        result,
        Err(vectory::Error::Xml(_)) | Err(vectory::Error::Parsing(_))
    ));
}

#[test]
fn preserves_css_carriage_returns_while_mapping_token_positions() {
    let input = "<svg><style>/*x*/&#13;\n.foo { x: 1 }</style></svg>";
    let output = class_suffixer::rewrite(input, "_inject_0").expect("rewrites");
    assert_eq!(style_text(&output), "/*x*/\r\n.foo_inject_0 { x: 1 }");
}

#[test]
fn gated_corpus_class_suffixer_goldens_round_trip() {
    // The styles corpus from the writer's fixtures keeps exercising the
    // suffixer against real SVG bytes.
    let input = std::fs::read_to_string(
        manip_common::repo_spec_dir().join("fixtures/svg-style-attrs-input.svg"),
    )
    .expect("corpus fixture");
    let output = class_suffixer::rewrite(&input, "_inject_0").expect("rewrites");
    let repeated = class_suffixer::rewrite(&output, "_inject_0").expect("rewrites");
    manip_common::assert_xml_equivalent(&repeated, &output);
}
