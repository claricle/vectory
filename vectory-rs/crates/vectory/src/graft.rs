//! Port of lib/vectory/svg_subtree_graft.rb — replaces serialized root
//! content while retaining the host document envelope.
//!
//! The envelope (declaration, prolog comments/PIs, DOCTYPE before the root;
//! trailing whitespace/comments/PIs after it) is kept from the ORIGINAL
//! host bytes; only the root subtree is re-serialized. Grafted children are
//! sealed with the namespace declarations the host cannot supply — the
//! same seal the Ruby layer performs (`seal_namespaces`), including the
//! `xmlns=""` default-undeclaration when neither side supplies a default.

use crate::dom::{Attribute, Document, Element, Node};
use crate::error::Error;
use crate::namespace::namespace_declaration;
use crate::scan::{root_offset, trailing_envelope};

/// Replaces the host document's root content with the processed document's
/// root children and returns the recombined serialized document.
///
/// # Errors
///
/// Returns [`Error::Parsing`] with the stable Ruby messages when either
/// side is malformed or rootless.
pub fn replace_root_content(host_content: &str, processed_content: &str) -> Result<String, Error> {
    let host = parse_document(host_content, "host_content")?;
    let processed = parse_document(processed_content, "processed_content")?;

    let mut grafted = host;
    if let (Some(processed_root), Some(root)) = (processed.root.as_ref(), grafted.root.as_mut()) {
        let host_ns = in_scope_declarations_root(root);
        let processed_scope = scope_declarations(processed_root);
        let mut children: Vec<Node> = Vec::new();
        for node in &processed_root.children {
            if let Node::Element(element) = node {
                children.push(Node::Element(seal_namespaces(
                    element,
                    &processed_scope,
                    &host_ns,
                )));
            } else {
                children.push(node.clone());
            }
        }
        root.children = children;
    }

    let Some(offset) = root_offset(host_content) else {
        return Ok(crate::serialize::serialize_document(&grafted));
    };
    let prefix = &host_content[..offset];
    let tail = trailing_envelope(host_content);
    Ok(format!(
        "{prefix}{}{tail}",
        crate::serialize::serialize(&grafted, crate::serialize::Profile::RubyXml)
    ))
}

/// Parses and validates a document at the serialized boundary, with the
/// stable Ruby error labels.
fn parse_document(content: &str, label: &str) -> Result<Document, Error> {
    match Document::parse(content) {
        Ok(document) => {
            if document.root.is_some() {
                Ok(document)
            } else {
                Err(Error::Parsing(format!("{label} has no document root")))
            }
        }
        Err(Error::NoRoot) => Err(Error::Parsing(format!("{label} has no document root"))),
        Err(Error::Xml(detail)) => Err(Error::Parsing(format!("invalid {label} XML: {detail}"))),
        Err(other) => Err(other),
    }
}

/// The host root's in-scope namespace declarations as `(name, uri)` pairs
/// where `name` is `xmlns` or `xmlns:p` — the Ruby `host.root.namespaces`.
fn in_scope_declarations_root(root: &Element) -> Vec<(String, String)> {
    scope_declarations(root)
        .into_iter()
        .map(|(prefix, uri)| {
            let name = if prefix.is_empty() {
                "xmlns".to_string()
            } else {
                format!("xmlns:{prefix}")
            };
            (name, uri)
        })
        .collect()
}

/// Collects the (prefix, uri) declarations on one element in attribute
/// order (innermost-wins for redeclarations) — the in-scope set for its
/// children.
fn scope_declarations(element: &Element) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for attribute in &element.attributes {
        if let Some((prefix, uri)) = namespace_declaration(&attribute.name, &attribute.value) {
            if let Some(existing) = out.iter_mut().find(|(p, _)| *p == prefix) {
                existing.1 = uri;
            } else {
                out.push((prefix, uri));
            }
        }
    }
    out
}

/// Seals namespace declarations onto a grafted child: retain inherited
/// bindings only where the host cannot supply them. The default namespace
/// participates as an explicit `""` (undeclared) binding when nothing
/// declares it — the Ruby `{ "xmlns" => "" }.merge(node.namespaces)` merge.
fn seal_namespaces(
    element: &Element,
    inherited: &[(String, String)],
    host_namespaces: &[(String, String)],
) -> Element {
    let mut sealed = element.clone();

    // The in-scope set at the child: inherited declarations with the
    // child's own declarations overriding them (outermost-first order).
    let mut scope: Vec<(String, String)> = inherited.to_vec();
    for (prefix, uri) in scope_declarations(element) {
        if let Some(existing) = scope.iter_mut().find(|(p, _)| *p == prefix) {
            existing.1 = uri;
        } else {
            scope.push((prefix, uri));
        }
    }
    // The default namespace joins the merge as "" when absent.
    if !scope.iter().any(|(prefix, _)| prefix.is_empty()) {
        scope.push((String::new(), String::new()));
    }

    let declared_on_node: Vec<String> = element
        .attributes
        .iter()
        .filter_map(|a| namespace_declaration(&a.name, &a.value).map(|(prefix, _)| prefix))
        .collect();

    for (prefix, uri) in &scope {
        if declared_on_node.contains(prefix) {
            continue;
        }
        let name = if prefix.is_empty() {
            "xmlns".to_string()
        } else {
            format!("xmlns:{prefix}")
        };
        if host_namespaces
            .iter()
            .any(|(host_name, host_uri)| host_name == &name && host_uri == uri)
        {
            continue;
        }
        let attribute_name = name;
        if sealed
            .attributes
            .iter()
            .any(|a| a.name == attribute_name && a.value == *uri)
        {
            continue;
        }
        sealed.attributes.push(Attribute {
            name: attribute_name,
            value: uri.clone(),
            namespace_uri: Some(crate::namespace::XMLNS_NAMESPACE_URI.to_string()),
        });
    }
    sealed
}
