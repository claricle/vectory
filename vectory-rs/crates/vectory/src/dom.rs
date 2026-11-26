use std::fmt;

use crate::error::Error;

/// A parsed SVG/XML document: prolog nodes, the root element, epilog nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Nodes before the root element (declaration, DOCTYPE, comments, PIs,
    /// whitespace text).
    pub prolog: Vec<Node>,
    /// The root element (`svg` for SVG documents).
    pub root: Option<Element>,
    /// Nodes after the root element (comments, PIs, whitespace text).
    pub epilog: Vec<Node>,
}

impl Document {
    /// Builds an SVG `Document` from the given XML content.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the content is not well-formed XML or has no
    /// root element.
    pub fn parse(content: &str) -> Result<Self, Error> {
        crate::decode::decode(content)
    }

    /// Serializes the document with the given profile.
    #[must_use]
    pub fn serialize(&self, profile: crate::serialize::Profile) -> String {
        crate::serialize::serialize(self, profile)
    }

    /// Depth-first iteration over every element in the document.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        let mut all: Vec<&Element> = Vec::new();
        for node in self.prolog.iter() {
            collect_element_nodes(node, &mut all);
        }
        if let Some(root) = &self.root {
            collect_elements(root, &mut all);
        }
        for node in self.epilog.iter() {
            collect_element_nodes(node, &mut all);
        }
        all.into_iter()
    }
}

fn collect_elements<'a>(element: &'a Element, out: &mut Vec<&'a Element>) {
    out.push(element);
    for child in &element.children {
        collect_element_nodes(child, out);
    }
}

fn collect_element_nodes<'a>(node: &'a Node, out: &mut Vec<&'a Element>) {
    if let Node::Element(element) = node {
        collect_elements(element, out);
    }
}

/// Any tree node: element, text, CDATA, comment, processing instruction,
/// XML declaration, or DOCTYPE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(Text),
    /// A CDATA section; content is the raw text between the delimiters.
    CData(String),
    /// A comment; content is the raw text between `<!--` and `-->`.
    Comment(String),
    /// A processing instruction; `content` is everything between the target
    /// and `?>` (including its leading space when present).
    ProcessingInstruction {
        target: String,
        content: String,
    },
    /// The XML declaration; `raw` is the verbatim declaration content
    /// (everything between `<?` and `?>`).
    Declaration(XmlDeclaration),
    /// A DOCTYPE; content is the raw text between `<!DOCTYPE` and `>`.
    DocType(String),
}

/// Character data (an XML text node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(pub String);

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Text(value.to_string())
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Text(value)
    }
}

/// The XML declaration, preserved verbatim so serialization reproduces the
/// source bytes (including emfsvg's literal
/// `<?xml version="1.0"  encoding="UTF-8" standalone="no"?>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlDeclaration {
    pub raw: String,
}

impl XmlDeclaration {
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        attribute_value(&self.raw, "version")
    }

    #[must_use]
    pub fn encoding(&self) -> Option<&str> {
        attribute_value(&self.raw, "encoding")
    }

    #[must_use]
    pub fn standalone(&self) -> Option<&str> {
        attribute_value(&self.raw, "standalone")
    }
}

fn attribute_value<'a>(raw: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = raw.find(&needle)? + needle.len();
    let rest = &raw[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// An XML element with ORDERED attributes.
///
/// Attribute order is source order — serialization fidelity depends on it.
/// `name` is the raw qualified name as written (`svg:a`, `xmlns:xlink`);
/// `namespace_uri` is the resolved namespace (see [`crate::namespace`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub namespace_uri: Option<String>,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
}

impl Element {
    /// Creates an element with the given raw qualified name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Element {
            name: name.to_string(),
            namespace_uri: None,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// The local part of the qualified name (`svg:a` → `a`).
    #[must_use]
    pub fn local_name(&self) -> &str {
        local_part(&self.name)
    }

    /// The lexical prefix (`svg:a` → `svg`), if any.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        prefix_of(&self.name)
    }

    /// The value of the first attribute with the given raw name.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Child elements (skipping text, comments and other non-element nodes).
    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// Sets an attribute, preserving order; an existing attribute with the
    /// same name is updated in place, otherwise appended at the end.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some(existing) = self.attributes.iter_mut().find(|a| a.name == name) {
            existing.value = value.to_string();
        } else {
            self.attributes.push(Attribute {
                name: name.to_string(),
                value: value.to_string(),
                namespace_uri: None,
                lead: 1,
            });
        }
    }
}

/// An attribute with its raw qualified name and value.
///
/// Namespace declarations (`xmlns`, `xmlns:p`) are ordinary ordered
/// attributes for serialization; see [`Attribute::is_namespace_declaration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub namespace_uri: Option<String>,
    /// The whitespace quirk: the number of spaces the serializer writes
    /// before this attribute. Parsing always sets 1 (a parsed document's
    /// inter-attribute whitespace is not representable); emitters set the
    /// reference renderer's fragment-composition counts (2 at the raw
    /// families' double-space sites).
    pub lead: u8,
}

impl Attribute {
    /// Builds an attribute with the plain single-space lead.
    #[must_use]
    pub fn new(name: &str, value: &str) -> Self {
        Attribute {
            name: name.to_string(),
            value: value.to_string(),
            namespace_uri: None,
            lead: 1,
        }
    }

    /// The local part of the qualified name (`xlink:href` → `href`).
    #[must_use]
    pub fn local_name(&self) -> &str {
        local_part(&self.name)
    }

    /// The lexical prefix (`xlink:href` → `xlink`), if any.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        prefix_of(&self.name)
    }

    /// True for `xmlns` and `xmlns:*` declarations.
    #[must_use]
    pub fn is_namespace_declaration(&self) -> bool {
        self.name == "xmlns" || self.name.starts_with("xmlns:")
    }
}

fn local_part(name: &str) -> &str {
    match name.rsplit_once(':') {
        Some((_, local)) => local,
        None => name,
    }
}

fn prefix_of(name: &str) -> Option<&str> {
    name.split_once(':').map(|(prefix, _)| prefix)
}
