//! Shared helpers for the manipulation-parity tests (vectory-rs lane 2).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use vectory::{Document, Element, Node};

/// The repository's spec/ directory (the vectory corpus).
#[must_use]
pub fn repo_spec_dir() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../spec")).to_path_buf()
}

/// A fixture under spec/fixtures/svg_transform.
#[must_use]
pub fn transform_fixture(rel: &str) -> String {
    std::fs::read_to_string(repo_spec_dir().join("fixtures/svg_transform").join(rel))
        .unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

/// A file under spec/examples/svg.
#[must_use]
pub fn example(rel: &str) -> String {
    std::fs::read_to_string(repo_spec_dir().join("examples/svg").join(rel))
        .unwrap_or_else(|e| panic!("example {rel}: {e}"))
}

/// The canon form used by the Ruby suite's `be_xml_equivalent_to`
/// (canon): decode both sides, sort attributes by (namespace URI, name),
/// serialize with explicit end tags and stable text escaping, and drop
/// namespace declarations that are redundant against the in-scope binding
/// (canonical XML namespace reduction).
#[must_use]
pub fn canon(content: &str) -> String {
    let document = Document::parse(content).expect("canon input parses");
    let mut out = String::new();
    if let Some(root) = &document.root {
        let scope: Vec<(String, String)> = Vec::new();
        canon_element(&mut out, root, &scope);
    }
    out
}

fn canon_element(out: &mut String, element: &Element, scope: &[(String, String)]) {
    out.push('<');
    out.push_str(&element.name);
    // The in-scope set after this element's own declarations.
    let mut inner_scope: Vec<(String, String)> = scope.to_vec();
    let mut declarations: Vec<&vectory::Attribute> = element
        .attributes
        .iter()
        .filter(|a| a.is_namespace_declaration())
        .collect();
    declarations.sort_by(|a, b| a.name.cmp(&b.name));
    let mut kept_declarations: Vec<(String, String)> = Vec::new();
    for attribute in declarations {
        let value = &attribute.value;
        if let Some((_, existing)) = inner_scope.iter().find(|(name, _)| *name == attribute.name) {
            if existing == value {
                // Redundant redeclaration: canonical form drops it.
                continue;
            }
        }
        let prefix = attribute
            .name
            .strip_prefix("xmlns:")
            .unwrap_or("")
            .to_string();
        if let Some(entry) = inner_scope
            .iter_mut()
            .find(|(name, _)| decl_prefix(name) == prefix)
        {
            entry.1 = value.clone();
        } else {
            inner_scope.push((attribute.name.clone(), value.clone()));
        }
        kept_declarations.push((attribute.name.clone(), value.clone()));
    }
    let mut attributes: Vec<&vectory::Attribute> = element
        .attributes
        .iter()
        .filter(|a| !a.is_namespace_declaration())
        .collect();
    attributes.sort_by(|a, b| {
        let a_ns = format!("{:?}", a.namespace_uri);
        let b_ns = format!("{:?}", b.namespace_uri);
        (a_ns, a.name.clone()).cmp(&(b_ns, b.name.clone()))
    });
    for (name, value) in &kept_declarations {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape(out, value);
        out.push('"');
    }
    for attribute in attributes {
        out.push(' ');
        out.push_str(&attribute.name);
        out.push_str("=\"");
        escape(out, &attribute.value);
        out.push('"');
    }
    if element.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &element.children {
        canon_node(out, child, &inner_scope);
    }
    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
}

fn decl_prefix(name: &str) -> String {
    name.strip_prefix("xmlns:").unwrap_or("").to_string()
}

fn escape(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
}

fn canon_node(out: &mut String, node: &Node, scope: &[(String, String)]) {
    match node {
        Node::Element(element) => canon_element(out, element, scope),
        Node::Text(text) => {
            // Canon comparison ignores whitespace-only text (the Nokogiri
            // reference formatting vs the produced output indentation).
            if text.0.chars().all(char::is_whitespace) {
                return;
            }
            for c in text.0.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '\r' => out.push_str("&#13;"),
                    _ => out.push(c),
                }
            }
        }
        Node::CData(content) => {
            out.push_str("<![CDATA[");
            out.push_str(content);
            out.push_str("]]>");
        }
        Node::Comment(content) => {
            out.push_str("<!--");
            out.push_str(content);
            out.push_str("-->");
        }
        Node::ProcessingInstruction { target, content } => {
            out.push_str("<?");
            out.push_str(target);
            out.push_str(content);
            out.push_str("?>");
        }
        Node::Declaration(decl) => {
            out.push_str("<?");
            out.push_str(&decl.raw);
            out.push_str("?>");
        }
        Node::DocType(content) => {
            out.push_str("<!DOCTYPE ");
            out.push_str(content);
            out.push('>');
        }
    }
}

/// Asserts two XML documents are canonically equivalent.
#[track_caller]
pub fn assert_xml_equivalent(actual: &str, expected: &str) {
    let actual_canon = canon(actual);
    let expected_canon = canon(expected);
    assert_eq!(
        actual_canon, expected_canon,
        "canonical forms differ\n--- actual ---\n{actual}\n--- expected ---\n{expected}"
    );
}

/// Finds the first descendant element by local name.
#[must_use]
pub fn find_first<'a>(root: &'a Element, local: &str) -> Option<&'a Element> {
    if root.local_name() == local {
        return Some(root);
    }
    for child in &root.children {
        if let Node::Element(child_element) = child {
            if let Some(found) = find_first(child_element, local) {
                return Some(found);
            }
        }
    }
    None
}

/// All descendant elements by local name, document order, root excluded.
#[must_use]
pub fn find_all<'a>(root: &'a Element, local: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    let mut collected: Vec<&Element> = Vec::new();
    vectory::mutation::elements(root, &mut collected);
    for element in collected.into_iter().skip(1) {
        if element.local_name() == local {
            out.push(element);
        }
    }
    out
}

/// The text content of an element (all descendant text, Ruby `text`).
#[must_use]
pub fn text_of(element: &Element) -> String {
    let mut out = String::new();
    for child in &element.children {
        match child {
            Node::Text(text) => out.push_str(&text.0),
            Node::CData(content) => out.push_str(content),
            Node::Element(child_element) => out.push_str(&text_of(child_element)),
            _ => {}
        }
    }
    out
}

/// The serialized child stream of a style element (`children.map(&:to_xml).join`).
#[must_use]
pub fn style_children_xml(element: &Element) -> String {
    let mut out = String::new();
    for child in &element.children {
        match child {
            Node::Text(text) => out.push_str(&escape_text_node(&text.0)),
            Node::CData(content) => {
                out.push_str("<![CDATA[");
                out.push_str(content);
                out.push_str("]]>");
            }
            Node::Comment(content) => {
                out.push_str("<!--");
                out.push_str(content);
                out.push_str("-->");
            }
            Node::Element(child_element) => {
                let document = Document {
                    prolog: Vec::new(),
                    root: Some(child_element.clone()),
                    epilog: Vec::new(),
                };
                out.push_str(&vectory::serialize(&document, vectory::Profile::RubyXml));
            }
            _ => {}
        }
    }
    out
}

fn escape_text_node(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}
