//! Namespace handling mirroring lib/vectory/svg_namespace_binding.rb.
//!
//! Semantics carried over from the Ruby layer:
//!
//! * declarations track lexically with scope — inner redeclarations shadow
//!   outer ones, and `xmlns:p=""` undeclares within its subtree;
//! * elements resolve through the default namespace (`xmlns`), attributes
//!   only through explicit prefixes — unprefixed attributes have NO namespace
//!   (per the XML Namespaces spec, and exactly what the Ruby binding's
//!   `attributes_use_uri?` relies on);
//! * the `xml:` prefix binds to the XML namespace implicitly;
//! * remaps operate on namespace URIs, not lexical prefixes, so shadowing
//!   and repeats are unambiguous;
//! * validation reports unbound element/attribute prefixes
//!   (`validate_names` / `validate_attributes` parity — [`validate_prefixes`]);
//! * lexical names are always preserved for serialization.

use crate::error::Error;

/// The SVG namespace URI.
pub const SVG_NAMESPACE_URI: &str = "http://www.w3.org/2000/svg";

/// The implicit `xml:` namespace URI.
pub const XML_NAMESPACE_URI: &str = "http://www.w3.org/XML/1998/namespace";

/// The `xmlns:` namespace URI (xmlns declarations themselves).
pub const XMLNS_NAMESPACE_URI: &str = "http://www.w3.org/2000/xmlns/";

/// Splits a raw qualified name into `(prefix, local)`.
#[must_use]
pub fn split_qname(name: &str) -> (Option<&str>, &str) {
    match name.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, name),
    }
}

/// An in-scope namespace declaration set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamespaceScope {
    bindings: Vec<(String, String)>,
}

impl NamespaceScope {
    /// Declares `prefix` (empty string = default namespace) bound to `uri`.
    /// An empty `uri` undeclares the prefix.
    pub fn declare(&mut self, prefix: &str, uri: &str) {
        if let Some(binding) = self.bindings.iter_mut().find(|(p, _)| p == prefix) {
            binding.1 = uri.to_string();
        } else {
            self.bindings.push((prefix.to_string(), uri.to_string()));
        }
    }

    /// Resolves `prefix` in this scope. Empty `prefix` is the default
    /// namespace. Returns `None` when unbound or explicitly undeclared.
    #[must_use]
    pub fn resolve(&self, prefix: &str) -> Option<&str> {
        self.bindings
            .iter()
            .rev()
            .find(|(p, _)| p == prefix)
            .map(|(_, uri)| uri.as_str())
            .filter(|uri| !uri.is_empty())
    }
}

/// Resolves a prefix through the scope chain (innermost first), honoring the
/// implicit `xml:` binding.
#[must_use]
pub fn resolve_prefix<'a>(scopes: &'a [NamespaceScope], prefix: &str) -> Option<&'a str> {
    for scope in scopes.iter().rev() {
        if let Some(uri) = scope.resolve(prefix) {
            return Some(uri);
        }
    }
    if prefix == "xml" {
        return Some(XML_NAMESPACE_URI);
    }
    None
}

/// Reads a namespace declaration from an attribute name/value pair.
/// Returns `Some((prefix, uri))` where an empty prefix means the default
/// namespace and an empty `uri` means "undeclare".
#[must_use]
pub fn namespace_declaration(name: &str, value: &str) -> Option<(String, String)> {
    if name == "xmlns" {
        return Some((String::new(), value.to_string()));
    }
    if let Some(prefix) = name.strip_prefix("xmlns:") {
        return Some((prefix.to_string(), value.to_string()));
    }
    None
}

/// Element/attribute namespace-resolution parity checks with
/// `SvgNamespaceBinding#validate_names` and `#validate_attributes`.
///
/// # Errors
///
/// Returns [`Error::Xml`] when a prefixed element or attribute has no
/// resolved namespace binding.
pub fn validate_prefixes(document: &crate::dom::Document) -> Result<(), Error> {
    if let Some(root) = &document.root {
        validate_element_prefixes(root)?;
    }
    Ok(())
}

fn validate_element_prefixes(element: &crate::dom::Element) -> Result<(), Error> {
    if element.name.contains(':') && element.namespace_uri.is_none() {
        return Err(Error::Xml("unbound element prefix".to_string()));
    }
    for attribute in &element.attributes {
        if attribute.name.contains(':')
            && !attribute.is_namespace_declaration()
            && attribute.namespace_uri.is_none()
        {
            return Err(Error::Xml("unbound attribute prefix".to_string()));
        }
    }
    for child in &element.children {
        if let crate::dom::Node::Element(child_element) = child {
            validate_element_prefixes(child_element)?;
        }
    }
    Ok(())
}
