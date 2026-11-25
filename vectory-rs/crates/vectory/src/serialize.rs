//! Model → bytes with serializer fidelity.
//!
//! Two explicit profiles (never a compromise — see the crate docs for the
//! full derived rule set):
//!
//! * [`Profile::RubyXml`] — the vectory writer (`root.to_xml` via leptris):
//!   root subtree only, libxml2 escaping.
//! * [`Profile::EmfsvgSvgBuilder`] — emfsvg's renderer builder: whole
//!   document (verbatim declaration), SvgBuilder escaping.

use crate::dom::{Document, Element, Node};

/// The serialization profile: which Ruby producer's emission rules to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// vectory's writer semantics: `Leptris::XML.parse(content).root.to_xml`.
    RubyXml,
    /// emfsvg's renderer serializer semantics (`Emfsvg::SvgBuilder`).
    EmfsvgSvgBuilder,
}

/// Serializes the document with the given profile.
#[must_use]
pub fn serialize(document: &Document, profile: Profile) -> String {
    let mut out = String::new();
    match profile {
        Profile::RubyXml => {
            if let Some(root) = &document.root {
                write_element(&mut out, root, profile);
            }
        }
        Profile::EmfsvgSvgBuilder => {
            for node in &document.prolog {
                write_node(&mut out, node, profile);
            }
            if let Some(root) = &document.root {
                write_element(&mut out, root, profile);
            }
            for node in &document.epilog {
                write_node(&mut out, node, profile);
            }
        }
    }
    out
}

fn write_element(out: &mut String, element: &Element, profile: Profile) {
    out.push('<');
    out.push_str(&element.name);
    match profile {
        // RubyXml: libxml2 keeps namespace declarations in the node's
        // nsDef list, separate from properties, and emits nsDef first —
        // so xmlns/xmlns:* always precede other attributes (probe-verified
        // on leptris), at every element, not just the root.
        Profile::RubyXml => {
            for attribute in element.attributes.iter().filter(|a| a.is_namespace_declaration()) {
                write_attribute(out, attribute, profile);
            }
            for attribute in element.attributes.iter().filter(|a| !a.is_namespace_declaration())
            {
                write_attribute(out, attribute, profile);
            }
        }
        // EmfsvgSvgBuilder: the renderer passes attributes as given — pure
        // model order (emfsvg output carries xmlns mid-list and must stay
        // there byte-for-byte).
        Profile::EmfsvgSvgBuilder => {
            for attribute in &element.attributes {
                write_attribute(out, attribute, profile);
            }
        }
    }
    if element.children.is_empty() {
        // R3: self-closing form, no leading space (both profiles).
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &element.children {
        write_node(out, child, profile);
    }
    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
}

fn write_attribute(out: &mut String, attribute: &crate::dom::Attribute, profile: Profile) {
    out.push(' ');
    out.push_str(&attribute.name);
    out.push_str("=\"");
    escape_attribute(out, &attribute.value, profile);
    out.push('"');
}

fn write_node(out: &mut String, node: &Node, profile: Profile) {
    match node {
        Node::Element(element) => write_element(out, element, profile),
        Node::Text(text) => escape_text(out, &text.0, profile),
        // R5/R6/R7: CDATA, comments, PIs emit verbatim.
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

fn escape_text(out: &mut String, value: &str, profile: Profile) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            // R11: CR re-escapes (a literal CR cannot survive decode).
            '\r' if profile == Profile::RubyXml => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
}

fn escape_attribute(out: &mut String, value: &str, profile: Profile) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // R12: whitespace from character references re-escapes to stay
            // put (a literal TAB/LF/CR cannot survive decode normalization).
            '\t' if profile == Profile::RubyXml => out.push_str("&#9;"),
            '\n' if profile == Profile::RubyXml => out.push_str("&#10;"),
            '\r' if profile == Profile::RubyXml => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
}
