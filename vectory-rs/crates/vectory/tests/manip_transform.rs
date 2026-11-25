//! Port of spec/vectory/svg_transform_pipeline_spec.rb and the fixture-
//! driven parts of spec/vectory/svg_transform_integration_spec.rb — the
//! #80 transform-pipeline line: profiles, sequence/scope validation,
//! envelope-preserving grafts, and idempotence.
//!
//! Ruby-process-bound deviations (documented, not skipped): the
//! integration spec's `eval` of docs/reference/api.adoc and its Nokogiri
//! namespace-object assertions are inseparable from the Ruby process; the
//! same migration scenarios are gated here against the checked-in goldens
//! and manifest (pipeline_profiles.yml) with canon equivalence, and the
//! ISO-8859-1/UTF-16 envelope normalization contexts are gated in their
//! UTF-8 form (the byte-transcoding happens in the Ruby process before the
//! serialized boundary this crate models).

mod manip_common;

use std::collections::HashMap;

use manip_common::{assert_xml_equivalent, find_first, transform_fixture};
use vectory::transform::{Profile, SvgTransform, SvgTransformPipeline};

fn disambiguate(index: usize, ids: &[&str]) -> SvgTransform {
    let mut map = HashMap::new();
    for id in ids {
        map.insert(id.to_string(), 1);
    }
    SvgTransform::disambiguate(index, map)
}

fn namespaces(bind: &[(&str, &str)], unbind: &[&str], remap: &[(&str, &str)]) -> SvgTransform {
    SvgTransform::bind_namespaces(bind, unbind, remap).expect("valid namespace request")
}

fn cleanup(index: usize, ids: &[&str]) -> SvgTransformPipeline {
    SvgTransformPipeline::new(vec![
        disambiguate(index, ids),
        SvgTransform::suffix_classes(index),
        namespaces(
            &[("link", "http://www.w3.org/1999/xlink")],
            &["xlink", "unused"],
            &[("http://www.w3.org/1999/xlink", "link")],
        ),
    ])
    .expect("valid pipeline")
}

const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"shape\"><style>.label { fill: red }</style><g class=\"label\"/></svg>";

#[test]
fn exposes_only_the_three_executable_profiles() {
    let cleanup = cleanup(1, &["shape"]);
    assert_eq!(cleanup.profile(), Some(Profile::SvgCleanup));
    assert_eq!(cleanup.steps().len(), 3);
    assert_eq!(Profile::Graft.steps().len(), 1);
    assert_eq!(Profile::SvgCleanup.steps().len(), 3);
    assert_eq!(Profile::GraftThenSvgCleanup.steps().len(), 4);
}

#[test]
fn builds_new_values_without_extending_the_original_prefix() {
    let prefix = SvgTransformPipeline::new(vec![
        disambiguate(1, &["shape"]),
        SvgTransform::suffix_classes(1),
    ])
    .expect("valid prefix");
    let pipeline = prefix
        .clone()
        .then(vec![namespaces(
            &[("xlink", "http://www.w3.org/1999/xlink")],
            &[],
            &[],
        )])
        .expect("valid extension");
    assert_eq!(prefix.steps().len(), 2);
    assert_eq!(pipeline.steps().len(), 3);
}

#[test]
fn rejects_incomplete_prefixes_before_parsing_input() {
    let disambiguate_only =
        SvgTransformPipeline::new(vec![disambiguate(1, &["shape"])]).expect("valid");
    let error = disambiguate_only.call("broken XML").unwrap_err();
    assert!(format!("{error}").contains("incomplete profile"), "{error}");
    let two_steps = SvgTransformPipeline::new(vec![
        disambiguate(1, &["shape"]),
        SvgTransform::suffix_classes(1),
    ])
    .expect("valid");
    let error = two_steps.call("broken XML").unwrap_err();
    assert!(format!("{error}").contains("incomplete profile"));
}

#[test]
fn rejects_skipped_duplicate_reversed_and_non_leading_graft_steps() {
    let graft = SvgTransform::graft_into("<svg/>").expect("valid host");
    let disambiguate_step = disambiguate(1, &["shape"]);
    let namespaces_step = namespaces(&[("xlink", "http://www.w3.org/1999/xlink")], &[], &[]);
    // skipped: disambiguate >> namespaces
    assert!(
        SvgTransformPipeline::new(vec![disambiguate_step.clone(), namespaces_step.clone()])
            .is_err()
    );
    // duplicate
    assert!(SvgTransformPipeline::new(vec![disambiguate_step.clone(), disambiguate_step]).is_err());
    // reversed
    assert!(SvgTransformPipeline::new(vec![
        SvgTransform::suffix_classes(1),
        disambiguate(1, &["shape"])
    ])
    .is_err());
    // non-leading graft
    let cleanup = cleanup(1, &["shape"]);
    assert!(cleanup.then(vec![graft]).is_err());
}

#[test]
fn rejects_complete_host_input_for_a_per_svg_cleanup() {
    let cleanup = cleanup(1, &["shape"]);
    let error = cleanup.call("<host><svg/><svg/></host>").unwrap_err();
    assert!(
        format!("{error}").contains("execution-scope mismatch"),
        "{error}"
    );
}

#[test]
fn rejects_all_step_execution_with_a_complete_host_capture() {
    let graft = SvgTransform::graft_into("<host><svg/></host>").expect("valid host");
    let pipeline = SvgTransformPipeline::new(vec![graft])
        .expect("valid")
        .then(vec![
            disambiguate(1, &["shape"]),
            SvgTransform::suffix_classes(1),
            namespaces(&[], &[], &[]),
        ])
        .expect("valid pipeline");
    let error = pipeline.call(SVG).unwrap_err();
    assert!(
        format!("{error}").contains("requires an SVG host root"),
        "{error}"
    );
}

#[test]
fn uses_the_same_positional_suffix_for_ids_and_classes() {
    let cleanup = cleanup(1, &["shape"]);
    let result = cleanup.call(SVG).expect("processes");
    let document = vectory::Document::parse(&result).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert_eq!(root.attribute("id"), Some("shape_inject_1"));
    let g = find_first(root, "g").unwrap();
    assert_eq!(g.attribute("class"), Some("label_inject_1"));
    let style = find_first(root, "style").unwrap();
    let style_text = style
        .children
        .first()
        .map(|child| match child {
            vectory::Node::Text(text) => text.0.clone(),
            _ => String::new(),
        })
        .unwrap_or_default();
    assert!(style_text.contains(".label_inject_1"), "{style_text}");
}

#[test]
fn captures_host_text_independently_and_preserves_host_metadata() {
    let host = "<svg xmlns=\"http://www.w3.org/2000/svg\" data-host=\"retained\"><old/></svg>";
    let graft = SvgTransform::graft_into(host).expect("valid host");
    let pipeline = SvgTransformPipeline::new(vec![graft])
        .expect("valid")
        .then(vec![
            disambiguate(1, &["shape"]),
            SvgTransform::suffix_classes(1),
            namespaces(&[("xlink", "http://www.w3.org/1999/xlink")], &[], &[]),
        ])
        .expect("valid pipeline");
    let result = pipeline.call(SVG).expect("processes");
    let document = vectory::Document::parse(&result).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert_eq!(pipeline.profile(), Some(Profile::GraftThenSvgCleanup));
    assert_eq!(root.attribute("data-host"), Some("retained"));
    assert!(find_first(root, "old").is_none());
    let g = find_first(root, "g").unwrap();
    assert_eq!(g.attribute("class"), Some("label_inject_1"));
    let xml = result.clone();
    assert!(
        xml.contains("xmlns:xlink=\"http://www.w3.org/1999/xlink\""),
        "{xml}"
    );
}

#[test]
fn allows_a_whole_document_graft_without_per_svg_operations() {
    let graft =
        SvgTransform::graft_into("<host retained=\"yes\"><old/></host>").expect("valid host");
    let pipeline = SvgTransformPipeline::new(vec![graft]).expect("valid");
    let result = pipeline
        .call("<mapped><svg/><svg/></mapped>")
        .expect("processes");
    let document = vectory::Document::parse(&result).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert_eq!(root.name, "host");
    assert_eq!(root.attribute("retained"), Some("yes"));
    assert_eq!(manip_common::find_all(root, "svg").len(), 2);
}

#[test]
fn rejects_malformed_captured_hosts_when_creating_a_step() {
    let error = SvgTransform::graft_into("<svg>").unwrap_err();
    assert!(
        matches!(error, vectory::Error::Parsing(detail) if detail.contains("invalid host_content XML"))
    );
}

#[test]
fn propagates_malformed_selected_input_before_processing_steps() {
    let cleanup = cleanup(1, &["shape"]);
    let error = cleanup.call("<svg>").unwrap_err();
    assert!(
        matches!(error, vectory::Error::Parsing(detail) if detail.contains("invalid content XML"))
    );
}

// ---------------------------------------------------------------------------
// svg_transform_integration_spec.rb — the manifest-driven goldens.
// ---------------------------------------------------------------------------

#[test]
fn pins_exactly_the_public_executable_profiles_and_their_sequences() {
    // pipeline_profiles.yml steps, verbatim.
    assert_eq!(
        Profile::Graft.steps(),
        &[vectory::transform::StepName::GraftInto]
    );
    assert_eq!(
        Profile::SvgCleanup.steps(),
        &[
            vectory::transform::StepName::Disambiguate,
            vectory::transform::StepName::SuffixClasses,
            vectory::transform::StepName::BindNamespaces,
        ]
    );
    assert_eq!(
        Profile::GraftThenSvgCleanup.steps(),
        &[
            vectory::transform::StepName::GraftInto,
            vectory::transform::StepName::Disambiguate,
            vectory::transform::StepName::SuffixClasses,
            vectory::transform::StepName::BindNamespaces,
        ]
    );
}

fn manifest_pipeline(name: &str) -> SvgTransformPipeline {
    let steps: Vec<SvgTransform> = match name {
        "graft" => vec![
            SvgTransform::graft_into(&transform_fixture("pipeline_inputs/host.xml")).expect("host"),
        ],
        "svg_cleanup" | "graft_then_svg_cleanup" => {
            let mut all: Vec<SvgTransform> = Vec::new();
            if name == "graft_then_svg_cleanup" {
                all.push(
                    SvgTransform::graft_into(&transform_fixture("pipeline_inputs/svg-host.svg"))
                        .expect("host"),
                );
            }
            all.push(disambiguate(1, &["drawing", "paint"]));
            all.push(SvgTransform::suffix_classes(1));
            all.push(namespaces(
                &[("link", "http://www.w3.org/1999/xlink")],
                &["xlink", "unused"],
                &[("http://www.w3.org/1999/xlink", "link")],
            ));
            all
        }
        other => panic!("unknown profile {other}"),
    };
    SvgTransformPipeline::new(steps).expect("manifest pipeline")
}

#[test]
fn each_profile_matches_its_checked_in_golden_and_reparses() {
    for (name, input, output) in [
        (
            "graft",
            "pipeline_inputs/mapped-host.xml",
            "pipeline_outputs/graft.xml",
        ),
        (
            "svg_cleanup",
            "pipeline_inputs/selected.svg",
            "pipeline_outputs/svg_cleanup.svg",
        ),
        (
            "graft_then_svg_cleanup",
            "pipeline_inputs/selected.svg",
            "pipeline_outputs/graft_then_svg_cleanup.svg",
        ),
    ] {
        let pipeline = manifest_pipeline(name);
        assert_eq!(
            pipeline.profile().map(|profile| profile.to_string()),
            Some(name.to_string())
        );
        let output_xml = pipeline.call(&transform_fixture(input)).expect("processes");
        assert_xml_equivalent(&output_xml, &transform_fixture(output));
        let document = vectory::Document::parse(&output_xml);
        assert!(document.is_ok(), "{name} output reparses");
        // Idempotent with the same captured inputs.
        let repeated = pipeline.call(&output_xml).expect("repeats");
        assert_xml_equivalent(&repeated, &output_xml);
    }
}

#[test]
fn graft_profile_retains_captured_root_metadata_while_replacing_content() {
    let pipeline = manifest_pipeline("graft");
    let output = pipeline
        .call(&transform_fixture("pipeline_inputs/mapped-host.xml"))
        .expect("processes");
    let document = vectory::Document::parse(&output).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert_eq!(root.name, "article");
    assert_eq!(root.namespace_uri.as_deref(), Some("urn:host"));
    assert_eq!(root.attribute("id"), Some("original"));
    assert_eq!(root.attribute("meta:owner"), Some("editor"));
    assert_eq!(root.attribute("xml:lang"), Some("en"));
    assert_eq!(root.attribute("discarded"), None);
    assert!(find_first(root, "old").is_none());
    let svg_ids: Vec<Option<&str>> = manip_common::find_all(root, "svg")
        .iter()
        .map(|svg| svg.attribute("id"))
        .collect();
    assert_eq!(svg_ids, vec![Some("one"), Some("two")]);
    let g = find_first(root, "g").unwrap();
    assert_eq!(g.attribute("class"), Some("plain"));
    assert!(output.contains("<!--mapped content-->"));
}

#[test]
fn cleanup_profiles_retain_rewritten_references_classes_and_final_namespaces() {
    for name in ["svg_cleanup", "graft_then_svg_cleanup"] {
        let pipeline = manifest_pipeline(name);
        let output = pipeline
            .call(&transform_fixture("pipeline_inputs/selected.svg"))
            .expect("processes");
        let document = vectory::Document::parse(&output).expect("reparses");
        let root = document.root.as_ref().unwrap();
        assert_eq!(root.attribute("id"), Some("drawing_inject_1"), "{name}");
        assert_eq!(
            root.attribute("class"),
            Some("drawing_inject_1 active_inject_1")
        );
        assert!(output.contains("xmlns:link=\"http://www.w3.org/1999/xlink\""));
        assert!(!output.contains("xmlns:xlink"));
        assert!(!output.contains("xmlns:unused"));
        let gradient = find_first(root, "linearGradient").unwrap();
        assert_eq!(gradient.attribute("id"), Some("paint_inject_1"));
        let g = find_first(root, "g").unwrap();
        assert_eq!(g.attribute("fill"), Some("url(#paint_inject_1)"));
        let use_element = find_first(root, "use").unwrap();
        assert_eq!(
            use_element.attribute("link:href"),
            Some("#drawing_inject_1")
        );
        let a = find_first(root, "a").unwrap();
        assert_eq!(a.attribute("href"), Some("#drawing_inject_1"));
        let rect = find_first(root, "rect").unwrap();
        assert_eq!(rect.attribute("class"), Some("active_inject_1"));
        // Nested selectors and non-selector CSS content.
        let style = find_first(root, "style").unwrap();
        let css = manip_common::text_of(style);
        assert!(
            css.contains(".drawing_inject_1.active_inject_1:hover"),
            "{css}"
        );
        assert!(css.contains(".drawing_inject_1:is(.active_inject_1, :not(.hidden_inject_1))"));
        assert!(css.contains("@supports"));
        assert!(css.contains("url(#paint_inject_1)"));
        assert!(css.contains("content: \".active\""));
        assert!(css.contains("/* .drawing stays */"));
        if name == "graft_then_svg_cleanup" {
            assert_eq!(root.attribute("data-owner"), Some("original"));
            assert_eq!(root.attribute("viewBox"), Some("0 0 40 20"));
            assert_eq!(root.attribute("xml:lang"), Some("en"));
            assert!(find_first(root, "old").is_none());
        }
    }
}

#[test]
fn grafts_a_real_whole_host_svgmap_expansion_before_selecting_svgs() {
    // svgmap-original.xml is processed by SvgMapping, grafted into the
    // original host, then the expansion output feeds the pipeline.
    let host = transform_fixture("consumer_flow/svgmap-original.xml");
    let expected_mapped = transform_fixture("consumer_flow/svgmap-mapped.xml");
    let spec_dir = manip_common::repo_spec_dir();
    let base = spec_dir.join("fixtures/svg_transform/consumer_flow");
    let mut mapping =
        vectory::svg_mapping::SvgMapping::from_xml(&host, &base, None).expect("mapping");
    let mapped = mapping.to_xml().expect("maps");
    assert_xml_equivalent(&mapped, &expected_mapped);

    let graft = SvgTransform::graft_into(&host).expect("host");
    let pipeline = SvgTransformPipeline::new(vec![graft]).expect("pipeline");
    let result = pipeline.call(&mapped).expect("processes");
    let document = vectory::Document::parse(&result).expect("reparses");
    let root = document.root.as_ref().unwrap();
    assert!(manip_common::find_all(root, "svgmap").is_empty());
    assert_eq!(root.attribute("id"), Some("mapping-host"));
    assert_eq!(root.attribute("data-owner"), Some("original"));
    assert_eq!(root.attribute("xml:lang"), Some("en"));
    let figure_svgs: Vec<Option<&str>> = manip_common::find_all(root, "figure")
        .iter()
        .flat_map(|figure| manip_common::find_all(figure, "svg"))
        .map(|svg| svg.attribute("id"))
        .collect();
    assert_eq!(
        figure_svgs,
        vec![Some("drawing_000000000"), Some("drawing_000000001")]
    );
    let a = find_first(root, "a").unwrap();
    assert_eq!(a.attribute("href"), Some("#chapter"));
}

#[test]
fn batch_disambiguation_gates_the_consumer_flow_corpus() {
    let mapped = transform_fixture("consumer_flow/mapped.xml");
    let document = vectory::Document::parse(&mapped).expect("reparses");
    let root = document.root.as_ref().unwrap();
    // `/article/figure/*[local-name()='svg']` — the figure-level SVGs only.
    let mut top_level: Vec<&vectory::Element> = Vec::new();
    for figure in manip_common::find_all(root, "figure") {
        for child in &figure.children {
            if let vectory::Node::Element(child_element) = child {
                if child_element.local_name() == "svg" {
                    top_level.push(child_element);
                }
            }
        }
    }
    let svgs: Vec<String> = top_level
        .iter()
        .map(|svg| {
            vectory::serialize(
                &vectory::Document {
                    prolog: Vec::new(),
                    root: Some((*svg).clone()),
                    epilog: Vec::new(),
                },
                vectory::Profile::RubyXml,
            )
        })
        .collect();
    let refs: Vec<&str> = svgs.iter().map(String::as_str).collect();
    let duplicates = vectory::id_disambiguator::duplicate_ids(&refs).expect("counts");
    let mut expected = HashMap::new();
    expected.insert("shared-root".to_string(), 2);
    expected.insert("paint".to_string(), 2);
    assert_eq!(duplicates, expected);
    let outputs: Vec<String> = svgs
        .iter()
        .enumerate()
        .map(|(index, svg)| {
            vectory::id_disambiguator::rewrite(svg, &format!("_inject_{index}"), &duplicates)
                .expect("rewrites")
        })
        .collect();
    assert_xml_equivalent(
        &outputs[0],
        &transform_fixture("consumer_flow/batch-first.svg"),
    );
    assert_xml_equivalent(
        &outputs[1],
        &transform_fixture("consumer_flow/batch-second.svg"),
    );
}

#[test]
fn grafted_svg_envelope_and_namespace_remap_keeps_href_and_fill() {
    let host = concat!(
        "<?xml version=\"1.0\"?><!-- lead --><!DOCTYPE svg>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" id=\"host\"><title>old</title></svg><!-- trail -->"
    );
    let graft = SvgTransform::graft_into(host).expect("host");
    let source = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">",
        "<style>.shape { fill: red }</style><defs><linearGradient id=\"g\"/></defs>",
        "<rect id=\"r\" xlink:href=\"#g\" fill=\"url(#g)\" class=\"shape\"/></svg>"
    );
    let pipeline = SvgTransformPipeline::new(vec![graft.clone()])
        .expect("valid")
        .then(vec![
            disambiguate(2, &["g", "r"]),
            SvgTransform::suffix_classes(2),
            namespaces(&[], &["xlink"], &[("http://www.w3.org/1999/xlink", "xl")]),
        ])
        .expect("valid pipeline");

    // Through the public graft step alone.
    let grafted = SvgTransformPipeline::new(vec![graft])
        .expect("valid")
        .call(source)
        .expect("processes");
    let document = vectory::Document::parse(&grafted).expect("reparses");
    let rect = find_first(document.root.as_ref().unwrap(), "rect").unwrap();
    assert_eq!(rect.attribute("xlink:href"), Some("#g"));
    assert_eq!(rect.attribute("fill"), Some("url(#g)"));

    // Through all four steps.
    let output = pipeline.call(source).expect("processes");
    let document = vectory::Document::parse(&output).expect("reparses");
    let rect = find_first(document.root.as_ref().unwrap(), "rect").unwrap();
    assert_eq!(rect.attribute("xl:href"), Some("#g_inject_2"));
    assert_eq!(rect.attribute("fill"), Some("url(#g_inject_2)"));
    // Captured envelope order.
    assert!(
        output.starts_with("<?xml version=\"1.0\"?><!-- lead --><!DOCTYPE svg>"),
        "{output}"
    );
    assert!(output.ends_with("<!-- trail -->"));
    // Idempotent.
    let repeated = pipeline.call(&output).expect("repeats");
    assert_eq!(repeated, output);
}

#[test]
fn preserves_descendant_namespace_shadows_and_default_undeclarations_on_repeat() {
    let host = concat!(
        "<?xml version=\"1.0\"?><!-- lead --><!DOCTYPE svg>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" id=\"host\"><title>old</title></svg><!-- trail -->"
    );
    let graft = SvgTransform::graft_into(host).expect("host");
    let source = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">",
        "<style>.shape { fill: red }</style><defs><linearGradient id=\"g\"/></defs>",
        "<rect id=\"r\" xlink:href=\"#g\" fill=\"url(#g)\" class=\"shape\"/>",
        "<g xmlns=\"urn:foreign\"><g xmlns=\"urn:foreign\"><plain xmlns=\"\"/></g></g></svg>"
    );
    let pipeline = SvgTransformPipeline::new(vec![graft])
        .expect("valid")
        .then(vec![
            disambiguate(2, &["g", "r"]),
            SvgTransform::suffix_classes(2),
            namespaces(&[], &["xlink"], &[("http://www.w3.org/1999/xlink", "xl")]),
        ])
        .expect("valid pipeline");
    let first = pipeline.call(source).expect("processes");
    // Name/namespace pairs survive (shadows and undeclarations intact).
    let names = |xml: &str| -> Vec<(String, Option<String>)> {
        let document = vectory::Document::parse(xml).expect("reparses");
        let mut collected: Vec<&vectory::Element> = Vec::new();
        vectory::mutation::elements(document.root.as_ref().unwrap(), &mut collected);
        collected
            .iter()
            .map(|element| (element.name.clone(), element.namespace_uri.clone()))
            .collect()
    };
    assert_eq!(names(&first), names(source));
    let repeated = pipeline.call(&first).expect("repeats");
    assert_eq!(repeated, first);
}

#[test]
fn utf8_declared_envelope_normalizes_through_the_pipeline() {
    // The encoding-normalization class of the integration spec, in its
    // UTF-8 form: the envelope is kept byte-for-byte and the document is
    // idempotent under the pipeline.
    let host = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--été--><svg id=\"r\" class=\"café\"/><!--fin-->";
    let graft = SvgTransform::graft_into(host).expect("host");
    let pipeline = SvgTransformPipeline::new(vec![graft])
        .expect("valid")
        .then(vec![
            disambiguate(1, &["r"]),
            SvgTransform::suffix_classes(1),
            namespaces(&[], &[], &[]),
        ])
        .expect("valid pipeline");
    let output = pipeline
        .call("<svg><text>été 日本語</text></svg>")
        .expect("processes");
    assert!(
        output.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--été-->"),
        "{output}"
    );
    assert!(output.ends_with("<!--fin-->"));
    assert!(output.contains("id=\"r_inject_1\""));
    assert!(output.contains("café_inject_1"));
    let repeated = pipeline.call(&output).expect("repeats");
    assert_eq!(repeated, output);
}
