//! Port of lib/vectory/svg_namespace_binding.rb — declaration changes on
//! one serialized SVG. Remaps use namespace URIs, rather than lexical
//! source prefixes, so shadows and repeats are unambiguous. Existing
//! aliases remain unless explicitly requested in `unbind`.

use crate::dom::{Document, Element, Node};
use crate::error::Error;
use crate::mutation::document_elements_ref;

/// A validated bind/unbind/remap request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BindingRequest {
    pub bind: Vec<(String, String)>,
    pub unbind: Vec<String>,
    /// (uri, prefix) — remap the URI to a new prefix (empty prefix = the
    /// default namespace).
    pub remap: Vec<(String, String)>,
}

impl BindingRequest {
    /// Validates the request with the Ruby layer's errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transform`] on invalid prefixes or URIs and on
    /// incompatible requested bindings.
    pub fn new(
        bind: &[(&str, &str)],
        unbind: &[&str],
        remap: &[(&str, &str)],
    ) -> Result<Self, Error> {
        let mut out = BindingRequest::default();
        for (prefix, uri) in bind {
            let prefix = normalize_prefix(prefix)?;
            let uri = validate_uri(uri)?;
            add_target(&mut out.bind, prefix, uri)?;
        }
        for prefix in unbind {
            let prefix = normalize_prefix(prefix)?;
            if !out.unbind.contains(&prefix) {
                out.unbind.push(prefix);
            }
        }
        for (uri, prefix) in remap {
            let uri = validate_uri(uri)?;
            let prefix = normalize_prefix(prefix)?;
            if !out.remap.iter().any(|(existing, _)| *existing == uri) {
                out.remap.push((uri, prefix));
            }
        }
        Ok(out)
    }
}

fn normalize_prefix(prefix: &str) -> Result<String, Error> {
    if prefix.is_empty() {
        return Ok(String::new());
    }
    let valid = prefix
        .chars()
        .enumerate()
        .all(|(index, c)| {
            if index == 0 {
                c.is_ascii_alphabetic() || c == '_'
            } else {
                c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
            }
        })
        && !prefix.to_lowercase().starts_with("xml");
    if !valid {
        return Err(Error::Transform(
            "invalid namespace request: invalid or reserved prefix".to_string(),
        ));
    }
    Ok(prefix.to_string())
}

fn validate_uri(uri: &str) -> Result<String, Error> {
    if uri.is_empty()
        || uri.chars().any(char::is_whitespace)
        || uri == "http://www.w3.org/XML/1998/namespace"
        || uri == "http://www.w3.org/2000/xmlns/"
    {
        return Err(Error::Transform(
            "invalid namespace request: expected nonempty namespace URI".to_string(),
        ));
    }
    Ok(uri.to_string())
}

fn add_target(
    targets: &mut Vec<(String, String)>,
    prefix: String,
    uri: String,
) -> Result<(), Error> {
    if let Some(existing) = targets.iter().find(|(p, _)| *p == prefix) {
        if existing.1 != uri {
            return Err(Error::Transform(
                "namespace conflict: incompatible requested bindings".to_string(),
            ));
        }
        return Ok(());
    }
    targets.push((prefix, uri));
    Ok(())
}

fn declaration_name(prefix: &str) -> String {
    if prefix.is_empty() {
        "xmlns".to_string()
    } else {
        format!("xmlns:{prefix}")
    }
}

/// Rewrites the namespace declarations of one serialized SVG.
///
/// # Errors
///
/// Returns [`Error`] on malformed input, a non-SVG root, unbound prefixes,
/// conflicts, or in-use unbind targets.
pub fn rewrite(content: &str, request: &BindingRequest) -> Result<String, Error> {
    let mut document = Document::parse(content).map_err(|error| match error {
        Error::NoRoot => Error::Transform("invalid SVG: expected SVG root".to_string()),
        Error::Xml(detail) => Error::Transform(format!("invalid SVG: {detail}")),
        other => other,
    })?;
    let Some(root) = &document.root else {
        return Err(Error::Transform("invalid SVG: expected SVG root".to_string()));
    };
    if root.name != "svg" {
        return Err(Error::Transform("invalid SVG: expected SVG root".to_string()));
    }
    let elements = document_elements_ref(&document);
    for element in &elements {
        if element.name.contains(':') && element.namespace_uri.is_none() {
            return Err(Error::Transform("invalid SVG: unbound element prefix".to_string()));
        }
        for attribute in &element.attributes {
            if attribute.name.contains(':') && attribute.namespace_uri.is_none() {
                return Err(Error::Transform(
                    "invalid SVG: unbound attribute prefix".to_string(),
                ));
            }
        }
    }
    drop(elements);

    // Targets: the bind requests plus the remap destinations.
    let mut targets = request.bind.clone();
    for (uri, prefix) in &request.remap {
        add_target(&mut targets, prefix.clone(), uri.clone())?;
    }

    for (prefix, uri) in &targets {
        preflight_declarations(&document, prefix, uri)?;
        if prefix.is_empty() {
            preflight_default(&document, uri, &request.remap)?;
        }
    }
    preflight_unbind(&document, request)?;

    for (prefix, uri) in &targets {
        bind_target(&mut document, prefix, uri);
    }
    remap_elements(&mut document, request);
    unbind_namespaces(&mut document, request);
    Ok(crate::serialize::serialize_document(&document))
}

fn preflight_declarations(document: &Document, prefix: &str, uri: &str) -> Result<(), Error> {
    for element in document_elements_ref(document) {
        for attribute in &element.attributes {
            if let Some((declared, declared_uri)) =
                crate::namespace::namespace_declaration(&attribute.name, &attribute.value)
            {
                if declared == prefix && declared_uri != uri {
                    return Err(Error::Transform(
                        "namespace conflict: target prefix already bound".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn preflight_default(
    document: &Document,
    uri: &str,
    remap: &[(String, String)],
) -> Result<(), Error> {
    let elements = document_elements_ref(document);
    if elements.iter().any(|element| element.namespace_uri.is_none()) {
        return Err(Error::Transform(
            "default namespace: would qualify unqualified elements".to_string(),
        ));
    }
    let remapped_to_default = remap.iter().any(|(existing, prefix)| existing == uri && prefix.is_empty());
    if remapped_to_default
        && elements.iter().any(|element| {
            element
                .attributes
                .iter()
                .any(|attr| attr.namespace_uri.as_deref() == Some(uri))
        })
    {
        return Err(Error::Transform(
            "default namespace: attributes require an explicit prefix".to_string(),
        ));
    }
    Ok(())
}

fn preflight_unbind(document: &Document, request: &BindingRequest) -> Result<(), Error> {
    for prefix in &request.unbind {
        if request.bind.iter().any(|(bound, _)| bound == prefix)
            || request.remap.iter().any(|(_, target)| target == prefix)
        {
            return Err(Error::Transform(
                "namespace conflict: target also requested for unbind".to_string(),
            ));
        }
        preflight_prefix_usage(document, prefix, request)?;
    }
    Ok(())
}

fn preflight_prefix_usage(document: &Document, prefix: &str, request: &BindingRequest) -> Result<(), Error> {
    for element in document_elements_ref(document) {
        let element_uses = matches!(element.prefix(), Some(element_prefix)
            if element_prefix == prefix
                && !request.remap.iter().any(|(uri, _)| element.namespace_uri.as_deref() == Some(uri)));
        if element_uses {
            return Err(Error::Transform(
                "namespace in use: cannot remove requested prefix".to_string(),
            ));
        }
        for attribute in &element.attributes {
            let attribute_uses = matches!(
                attribute.prefix(),
                Some(attribute_prefix)
                if attribute_prefix == prefix
                    && attribute.namespace_uri.is_some()
                    && !request
                        .remap
                        .iter()
                        .any(|(uri, _)| attribute.namespace_uri.as_deref() == Some(uri.as_str()))
            );
            if attribute_uses {
                return Err(Error::Transform(
                    "namespace in use: cannot remove requested prefix".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn bind_target(document: &mut Document, prefix: &str, uri: &str) {
    let Some(root) = document.root.as_mut() else {
        return;
    };
    let name = declaration_name(prefix);
    let current = current_binding(root, &name);
    if current.as_deref() == Some(uri) {
        return;
    }
    root.attributes.push(crate::dom::Attribute {
        name,
        value: uri.to_string(),
        namespace_uri: Some(crate::namespace::XMLNS_NAMESPACE_URI.to_string()),
    });
}

/// The root's current binding for a declaration name: its own declaration,
/// else the inherited default (for a parsed SVG document the root has no
/// inherited scope, so this is the root's own declarations only — matching
/// `root.namespaces` on a document root).
fn current_binding(root: &Element, name: &str) -> Option<String> {
    root.attributes
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.value.clone())
}

fn remap_elements(document: &mut Document, request: &BindingRequest) {
    let Some(root) = document.root.as_mut() else {
        return;
    };
    remap_element(root, request);
}

fn remap_element(element: &mut Element, request: &BindingRequest) {
    // Snapshot before changing names: namespace resolution is lexical.
    let element_uri = element.namespace_uri.clone();
    let element_name = element.name.clone();
    if let Some(uri) = element_uri {
        if let Some((_, prefix)) = request.remap.iter().find(|(existing, _)| *existing == uri) {
            element.name = qualified_name(prefix, &element_name);
        }
    }
    let snapshot: Vec<(usize, String, String, String)> = element
        .attributes
        .iter()
        .enumerate()
        .map(|(index, attribute)| {
            (
                index,
                attribute.namespace_uri.clone().unwrap_or_default(),
                attribute.name.rsplit(':').next().unwrap_or("").to_string(),
                attribute.value.clone(),
            )
        })
        .collect();
    let in_remap =
        |uri: &str| !uri.is_empty() && request.remap.iter().any(|(existing, _)| existing == uri);
    let remapped: Vec<(String, String, String)> = snapshot
        .iter()
        .filter(|(_, uri, _, _)| in_remap(uri))
        .map(|(_, uri, local, value)| (uri.clone(), local.clone(), value.clone()))
        .collect();
    let remapped_indices: Vec<usize> = snapshot
        .iter()
        .filter(|(_, uri, _, _)| in_remap(uri))
        .map(|(index, _, _, _)| *index)
        .collect();
    for index in remapped_indices.into_iter().rev() {
        element.attributes.remove(index);
    }
    for (uri, local, value) in remapped {
        let prefix = &request
            .remap
            .iter()
            .find(|(existing, _)| *existing == uri)
            .unwrap()
            .1;
        let name = qualified_name(prefix, &local);
        if let Some(existing) = element.attributes.iter_mut().find(|a| a.name == name) {
            existing.value = value;
        } else {
            element.attributes.push(crate::dom::Attribute {
                name,
                value,
                namespace_uri: None,
            });
        }
    }

    for child in &mut element.children {
        if let Node::Element(child_element) = child {
            remap_element(child_element, request);
        }
    }
}

fn unbind_namespaces(document: &mut Document, request: &BindingRequest) {
    unbind_element_rec(document.root.as_mut(), request);
}

fn unbind_element_rec(element: Option<&mut Element>, request: &BindingRequest) {
    let Some(element) = element else {
        return;
    };
    element.attributes.retain(|attribute| {
        !attribute.is_namespace_declaration()
            || !matches!(
                crate::namespace::namespace_declaration(&attribute.name, &attribute.value),
                Some((prefix, _)) if request.unbind.contains(&prefix)
            )
    });
    for child in &mut element.children {
        if let Node::Element(child_element) = child {
            unbind_element_rec(Some(child_element), request);
        }
    }
}

fn qualified_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
