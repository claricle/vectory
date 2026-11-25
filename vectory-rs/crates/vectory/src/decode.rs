use std::borrow::Cow;

use quick_xml::escape::unescape;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;

use crate::dom::{Attribute, Document, Element, Node, Text, XmlDeclaration};
use crate::error::Error;
use crate::namespace::{namespace_declaration, resolve_prefix, NamespaceScope};

/// Decodes XML content into the SVG model.
///
/// Mirrors the vectory writer's parse (`Leptris::XML.parse(content)`):
/// line ends normalize to LF per the XML spec, attribute values normalize
/// per XML attribute-value normalization (literal TAB/LF/CR become spaces;
/// character references survive), CDATA stays a distinct node, and entity
/// references decode to characters.
///
/// # Errors
///
/// Returns [`Error::Xml`] on malformed input (unknown entities, mismatched
/// tags, elements after the root, non-whitespace text after the root) and
/// [`Error::NoRoot`] when there is no root element.
pub fn decode(content: &str) -> Result<Document, Error> {
    let mut reader = Reader::from_str(content);
    let mut scopes: Vec<NamespaceScope> = vec![NamespaceScope::default()];
    let mut prolog: Vec<Node> = Vec::new();
    let mut epilog: Vec<Node> = Vec::new();
    let mut root: Option<Element> = None;
    let mut stack: Vec<Element> = Vec::new();

    loop {
        let event = reader.read_event().map_err(Error::from)?;
        match event {
            Event::Start(start) => {
                let element = build_element(&start, &mut scopes)?;
                scopes.push(declarations_scope(&element.attributes));
                stack.push(element);
            }
            Event::Empty(start) => {
                let element = build_element(&start, &mut scopes)?;
                let node = Node::Element(element);
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::End(_) => {
                scopes.pop();
                let element = stack.pop().ok_or_else(|| {
                    Error::Xml("closing tag without matching opening tag".to_string())
                })?;
                let node = Node::Element(element);
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::Text(text) => {
                let content = text.xml10_content();
                let decoded = unescape(&content)?;
                let node = Node::Text(Text(decoded.into_owned()));
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::GeneralRef(reference) => {
                // quick-xml delivers `&ref;` as its own event; the entity
                // resolves into the surrounding text node (libxml2 keeps one
                // content node). `&gt;` in text must serialize back as
                // `&gt;` via the text escaping rules.
                let raw = format!("&{};", reference.into_inner());
                let decoded = unescape(&raw)?;
                let node = Node::Text(Text(decoded.into_owned()));
                attach_coalescing(node, &mut stack, &mut root, &mut prolog, &mut epilog);
            }
            Event::CData(cdata) => {
                let node = Node::CData(normalize_eols(&cdata.into_inner()).into_owned());
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::Comment(comment) => {
                let node = Node::Comment(normalize_eols(&comment.xml10_content()).into_owned());
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::PI(pi) => {
                let node = Node::ProcessingInstruction {
                    target: normalize_eols(pi.target()).into_owned(),
                    content: normalize_eols(pi.content()).into_owned(),
                };
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::Decl(decl) => {
                let node = Node::Declaration(XmlDeclaration {
                    raw: normalize_eols(&decl).into_owned(),
                });
                attach(node, &mut stack, &mut root, &mut prolog, &mut epilog)?;
            }
            Event::DocType(doctype) => {
                if root.is_some() || !stack.is_empty() {
                    return Err(Error::Xml("DOCTYPE after the root element".to_string()));
                }
                prolog.push(Node::DocType(normalize_eols(&doctype.xml10_content()).into_owned()));
            }
            Event::Eof => break,
        }
    }

    if !stack.is_empty() {
        return Err(Error::Xml("unclosed element(s) at end of input".to_string()));
    }
    let root = root.ok_or(Error::NoRoot)?;
    check_epilog(&epilog)?;
    Ok(Document {
        prolog,
        root: Some(root),
        epilog,
    })
}

fn check_epilog(epilog: &[Node]) -> Result<(), Error> {
    for node in epilog {
        if let Node::Text(Text(content)) = node {
            if !content.chars().all(char::is_whitespace) {
                return Err(Error::Xml(
                    "extra content at the end of the document".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn attach(
    node: Node,
    stack: &mut [Element],
    root: &mut Option<Element>,
    prolog: &mut Vec<Node>,
    epilog: &mut Vec<Node>,
) -> Result<(), Error> {
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
        return Ok(());
    }
    if let Node::Element(element) = node {
        if root.is_some() {
            return Err(Error::Xml(
                "extra content at the end of the document".to_string(),
            ));
        }
        *root = Some(element);
        return Ok(());
    }
    if root.is_some() {
        epilog.push(node);
    } else {
        prolog.push(node);
    }
    Ok(())
}

/// Attaches a text node, coalescing with an immediately preceding text node
/// (entity references resolve into the surrounding text content).
fn attach_coalescing(
    node: Node,
    stack: &mut [Element],
    root: &mut Option<Element>,
    prolog: &mut Vec<Node>,
    epilog: &mut Vec<Node>,
) {
    let Node::Text(Text(decoded)) = node else {
        unreachable!("attach_coalescing only handles text nodes");
    };
    if let Some(top) = stack.last_mut() {
        append_text(&mut top.children, decoded);
        return;
    }
    if root.is_some() {
        append_text(epilog, decoded);
    } else {
        append_text(prolog, decoded);
    }
}
fn append_text(nodes: &mut Vec<Node>, content: String) {
    if let Some(Node::Text(Text(existing))) = nodes.last_mut() {
        existing.push_str(&content);
    } else {
        nodes.push(Node::Text(Text(content)));
    }
}

fn build_element(
    start: &quick_xml::events::BytesStart<'_>,
    scopes: &mut Vec<NamespaceScope>,
) -> Result<Element, Error> {
    let name = start.name().as_ref().to_string();
    let mut attributes: Vec<Attribute> = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(Error::from)?;
        attributes.push(Attribute {
            name: attr.key.as_ref().to_string(),
            value: attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(Error::from)?
                .into_owned(),
            namespace_uri: None,
        });
    }

    // The element's own xmlns declarations take effect for the element
    // itself and its attributes (XML Namespaces: a default namespace
    // declared on an element applies to that element).
    scopes.push(declarations_scope(&attributes));
    let element_namespace_uri = match crate::namespace::split_qname(&name).0 {
        Some(prefix) => resolve_prefix(scopes, prefix).map(str::to_string),
        None => resolve_prefix(scopes, "").map(str::to_string),
    };
    for attribute in &mut attributes {
        if attribute.is_namespace_declaration() {
            attribute.namespace_uri = Some(crate::namespace::XMLNS_NAMESPACE_URI.to_string());
            continue;
        }
        if let Some(prefix) = crate::namespace::split_qname(&attribute.name).0 {
            attribute.namespace_uri = resolve_prefix(scopes, prefix).map(str::to_string);
        }
        // Unprefixed attributes have no namespace (XML Namespaces spec).
    }
    scopes.pop();

    Ok(Element {
        name,
        namespace_uri: element_namespace_uri,
        attributes,
        children: Vec::new(),
    })
}

fn declarations_scope(attributes: &[Attribute]) -> NamespaceScope {
    let mut scope = NamespaceScope::default();
    for attribute in attributes {
        if let Some((prefix, uri)) = namespace_declaration(&attribute.name, &attribute.value) {
            scope.declare(&prefix, &uri);
        }
    }
    scope
}

/// Normalizes XML line ends: `\r\n` and `\r` become `\n`.
pub(crate) fn normalize_eols(value: &str) -> Cow<'_, str> {
    if !value.contains('\r') {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}
