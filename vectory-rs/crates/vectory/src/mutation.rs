//! Mutation on the model: the operations SvgMapping and the transform
//! pipeline need, expressed once against [`Document`]/[`Element`].
//!
//! This is the Rust counterpart of the Ruby layer's DOM mutations
//! (`image.replace(content)`, `target_tag.remove`, `svgmap.replace(figure)`,
//! `root.children.each(&:remove)` + graft). The #84 contract — processing
//! writes back into the CALLER's document — is expressible here as plain
//! `&mut` operations on the caller's tree.

use crate::dom::{Document, Element, Node};

/// Immutable depth-first element collection, root included.
pub fn elements<'a>(element: &'a Element, out: &mut Vec<&'a Element>) {
    out.push(element);
    for child in &element.children {
        if let Node::Element(child_element) = child {
            elements(child_element, out);
        }
    }
}

pub fn document_elements_ref(document: &Document) -> Vec<&Element> {
    let mut out = Vec::new();
    if let Some(root) = document.root.as_ref() {
        elements(root, &mut out);
    }
    out
}

/// Replaces the document root's children with `children` — the write-back
/// graft the caller contract performs (`root.children.each(&:remove)` then
/// appending the processed root's children).
pub fn replace_root_children(document: &mut Document, children: Vec<Node>) {
    if let Some(root) = document.root.as_mut() {
        root.children = children;
    }
}

/// Moves the processed document's root children into the caller document's
/// root — the #84 write-back, on the model.
pub fn write_back(caller: &mut Document, processed: &Document) {
    let children = processed
        .root
        .as_ref()
        .map(|root| root.children.clone())
        .unwrap_or_default();
    replace_root_children(caller, children);
}

/// True when the element is in `namespace_uri` (None = no namespace) by
/// local name — the traversal predicate standing in for the Ruby layer's
/// namespace-qualified XPath.
pub fn is_named(element: &Element, namespace_uri: Option<&str>, local: &str) -> bool {
    element.local_name() == local && element.namespace_uri.as_deref() == namespace_uri
}
