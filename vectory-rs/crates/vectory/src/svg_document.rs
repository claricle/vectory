//! Port of lib/vectory/svg_document.rb — represents and processes an SVG
//! document within XML: two-stage ID suffixing (cross-document id suffix
//! first, then the per-svgmap index suffix), link remapping, and processing
//! instruction removal. Final ID format: `<original><id_suffix><index>`.

use crate::dom::{Document, Element, Node};
use crate::error::Error;
use crate::mutation::document_elements_ref;
use crate::namespace::SVG_NAMESPACE_URI;

/// The processed SVG document (a decoded model with the Ruby processing
/// order: remap links, apply id suffix, apply index suffix, remove the
/// matched elements).
pub struct SvgDocument {
    document: Document,
}

impl SvgDocument {
    /// Decodes SVG content.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed XML.
    pub fn new(content: &str) -> Result<Self, Error> {
        Ok(SvgDocument {
            document: Document::parse(content)?,
        })
    }

    /// The processed SVG content as XML (`root.to_xml`).
    #[must_use]
    pub fn content(&self) -> String {
        crate::serialize::serialize(&self.document, crate::serialize::Profile::RubyXml)
    }

    /// Applies the namespace processing: remap internal links to their
    /// targets, apply the optional id suffix (cross-document uniqueness),
    /// apply the index suffix (multi-svgmap uniqueness), remove the
    /// elements matched by the removal policy (processing instructions).
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed intermediate states.
    pub fn namespace(
        &mut self,
        index_suffix: usize,
        links: &[(String, String)],
        base_dir: &str,
        id_suffix: Option<&str>,
    ) -> Result<(), Error> {
        self.remap_links(links, base_dir);
        if let Some(id_suffix) = id_suffix {
            self.apply_suffix(id_suffix)?;
        }
        self.apply_index_suffix(index_suffix)?;
        self.remove_processing_instructions();
        Ok(())
    }

    /// Remaps internal SVG links (`<a>` in the SVG namespace) to their
    /// target definitions.
    pub fn remap_links(&mut self, map: &[(String, String)], base_dir: &str) {
        let Some(root) = self.document.root.as_mut() else {
            return;
        };
        remap_links_in(root, map, base_dir);
    }

    /// Applies a suffix to all SVG IDs (id attributes, href fragments,
    /// `url(#id)` references and CSS references). `index` is zero-padded to
    /// nine digits.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed intermediate states.
    pub fn apply_index_suffix(&mut self, index: usize) -> Result<(), Error> {
        self.apply_suffix(&format!("{index:09}"))
    }

    fn apply_suffix(&mut self, suffix: &str) -> Result<(), Error> {
        let Some(root) = self.document.root.as_mut() else {
            return Ok(());
        };
        let ids = collect_ids(root);
        if ids.is_empty() {
            return Ok(());
        }
        update_ids_attrs(root, &ids, suffix);
        update_ids_css(root, &ids, suffix);
        Ok(())
    }

    /// Removes the elements matched by the removal policy (processing
    /// instructions anywhere in the document).
    pub fn remove_processing_instructions(&mut self) {
        remove_pi_paths(&mut self.document);
    }

    /// Access for the caller-document write-back (SvgMapping replaces the
    /// image element with this document's serialized content).
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }
}

/// Removes every processing-instruction node, by path (reverse document
/// order keeps sibling paths valid).
fn remove_pi_paths(document: &mut Document) {
    let Some(root) = document.root.as_mut() else {
        return;
    };
    let mut paths: Vec<Vec<usize>> = Vec::new();
    collect_pi_paths(root, &mut Vec::new(), &mut paths);
    for path in paths.into_iter().rev() {
        if let Some((parent, index)) = split_path(root, &path) {
            parent.children.remove(index);
        }
    }
}

fn collect_pi_paths(element: &Element, current: &mut Vec<usize>, paths: &mut Vec<Vec<usize>>) {
    for (index, child) in element.children.iter().enumerate() {
        current.push(index);
        if matches!(child, Node::ProcessingInstruction { .. }) {
            paths.push(current.clone());
        }
        if let Node::Element(child_element) = child {
            collect_pi_paths(child_element, current, paths);
        }
        current.pop();
    }
}

fn split_path<'a>(root: &'a mut Element, path: &[usize]) -> Option<(&'a mut Element, usize)> {
    let index = *path.last()?;
    let parents = &path[..path.len() - 1];
    let mut cursor = root;
    for &step in parents {
        match cursor.children.get_mut(step) {
            Some(Node::Element(child)) => cursor = child,
            _ => return None,
        }
    }
    Some((cursor, index))
}

/// The SVG-namespace `<a>` link remapping, recursive.
fn remap_links_in(element: &mut Element, map: &[(String, String)], base_dir: &str) {
    if element.namespace_uri.as_deref() == Some(SVG_NAMESPACE_URI) && element.local_name() == "a" {
        for attribute in &mut element.attributes {
            if attribute.name == "xlink:href" || attribute.name == "href" {
                let expanded = crate::svg_mapping::expand_path(&attribute.value, base_dir);
                if let Some((_, target)) = map.iter().find(|(key, _)| *key == expanded) {
                    attribute.value = target.clone();
                }
            }
        }
    }
    for child in &mut element.children {
        if let Node::Element(child_element) = child {
            remap_links_in(child_element, map, base_dir);
        }
    }
}

/// All id attribute values (root's own `id` plus every descendant `id`).
fn collect_ids(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(id) = root.attribute("id") {
        out.push(id.to_string());
    }
    let mut descendants: Vec<&Element> = Vec::new();
    crate::mutation::elements(root, &mut descendants);
    for element in descendants.into_iter().skip(1) {
        if let Some(id) = element.attribute("id") {
            out.push(id.to_string());
        }
    }
    out
}

/// The Ruby `update_ids_attrs`: for the root and every descendant with any
/// attribute, each attribute value gets the suffix appended when it equals
/// an id after one leading `#`, and the first `url(#...)` reference gains
/// the suffix.
fn update_ids_attrs(root: &mut Element, ids: &[String], suffix: &str) {
    update_ids_attrs_in(root, ids, suffix);
}

fn update_ids_attrs_in(element: &mut Element, ids: &[String], suffix: &str) {
    for attribute in &mut element.attributes {
        let original = attribute.value.clone();
        let mut value = original.clone();
        let stripped = original.strip_prefix('#').unwrap_or(&original);
        if ids.iter().any(|id| id == stripped) {
            value = format!("{value}_{suffix}");
        }
        if let Some((start, end, id)) = first_url_reference(&value) {
            if ids.iter().any(|id_value| id_value == &id) {
                value = format!(
                    "{}{}_{}{}",
                    &value[..start],
                    &value[start..end],
                    suffix,
                    &value[end..]
                );
            }
        }
        attribute.value = value;
    }
    for child in &mut element.children {
        if let Node::Element(child_element) = child {
            update_ids_attrs_in(child_element, ids, suffix);
        }
    }
}

/// The first `url(#id)` reference: char range of the id and the id itself.
/// The Ruby regex id charset is `[^()]+`.
fn first_url_reference(value: &str) -> Option<(usize, usize, String)> {
    let chars: Vec<char> = value.chars().collect();
    for index in 0..chars.len() {
        if chars[index] == 'u'
            && chars.get(index + 1) == Some(&'r')
            && chars.get(index + 2) == Some(&'l')
            && chars.get(index + 3) == Some(&'(')
            && chars.get(index + 4) == Some(&'#')
        {
            let id_start = index + 5;
            let mut end = id_start;
            while chars.get(end).is_some_and(|c| *c != '(' && *c != ')') {
                end += 1;
            }
            if chars.get(end) == Some(&')') && end > id_start {
                return Some((id_start, end, chars[id_start..end].iter().collect()));
            }
        }
    }
    None
}

/// The Ruby `update_ids_css`: style elements in the SVG namespace get their
/// full child content rewritten as one text node; every `style` attribute
/// value is rewritten in place.
fn update_ids_css(root: &mut Element, ids: &[String], suffix: &str) {
    update_ids_css_in(root, ids, suffix);
}

fn update_ids_css_in(element: &mut Element, ids: &[String], suffix: &str) {
    if element.namespace_uri.as_deref() == Some(SVG_NAMESPACE_URI) && element.local_name() == "style" {
        let children_xml: String = element
            .children
            .iter()
            .map(|child| serialize_child_node(child))
            .collect();
        let updated = update_ids_css_string(&children_xml, ids, suffix);
        element.children = vec![Node::Text(crate::dom::Text(updated))];
    }
    for attribute in &mut element.attributes {
        if attribute.name == "style" {
            attribute.value = update_ids_css_string(&attribute.value, ids, suffix);
        }
    }
    for child in &mut element.children {
        if let Node::Element(child_element) = child {
            update_ids_css_in(child_element, ids, suffix);
        }
    }
}

fn serialize_child_node(node: &Node) -> String {
    match node {
        Node::Text(text) => {
            let mut out = String::new();
            for c in text.0.chars() {
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
        Node::CData(content) => format!("<![CDATA[{content}]]>"),
        Node::Comment(content) => format!("<!--{content}-->"),
        Node::ProcessingInstruction { target, content } => {
            format!("<?{target}{content}?>")
        }
        Node::Element(element) => crate::serialize::serialize(
            &Document {
                prolog: Vec::new(),
                root: Some(element.clone()),
                epilog: Vec::new(),
            },
            crate::serialize::Profile::RubyXml,
        ),
        _ => String::new(),
    }
}

/// The Ruby `update_ids_css_string`: `#id` word-boundary references and
/// `[id = 'id']` selector forms gain the suffix.
#[must_use]
pub fn update_ids_css_string(style: &str, ids: &[String], suffix: &str) -> String {
    let mut out = style.to_string();
    for id in ids {
        out = replace_hash_reference(&out, id, suffix);
        out = replace_attribute_selector(&out, id, suffix);
    }
    out
}

/// `#id\b` → `#id_suffix` (word boundary: the next char is not
/// alphanumeric/underscore).
fn replace_hash_reference(style: &str, id: &str, suffix: &str) -> String {
    let needle = format!("#{id}");
    let mut out = String::new();
    let mut rest = style;
    while let Some(position) = rest.find(&needle) {
        let after = rest[position + needle.len()..].chars().next();
        let boundary = after.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        out.push_str(&rest[..position + needle.len()]);
        rest = &rest[position + needle.len()..];
        if boundary {
            out.push('_');
            out.push_str(suffix);
        }
    }
    out.push_str(rest);
    out
}

/// `[id = 'id']` (any quote, optional spaces) → `[id='id_suffix']`.
fn replace_attribute_selector(style: &str, id: &str, suffix: &str) -> String {
    let mut out = String::new();
    let mut rest = style;
    while let Some(position) = rest.find("[id") {
        let after = &rest[position..];
        if let Some(end) = match_attribute_selector(after, id) {
            out.push_str(&rest[..position]);
            out.push_str(&format!("[id='{id}_{suffix}']"));
            rest = &rest[position + end..];
        } else {
            out.push_str(&rest[..position + 3]);
            rest = &rest[position + 3..];
        }
    }
    out.push_str(rest);
    out
}

/// Matches `[id <ws>* = <ws>* ['"]id['"] <ws>* ]` at the start; returns the
/// consumed length.
fn match_attribute_selector(text: &str, id: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, open) = chars.next()?;
    if open != '[' {
        return None;
    }
    let mut cursor = 1;
    let body: Vec<char> = text.chars().collect();
    // "id"
    for expected in "id".chars() {
        if body.get(cursor) != Some(&expected) {
            return None;
        }
        cursor += 1;
    }
    while matches!(body.get(cursor), Some(c) if c.is_whitespace()) {
        cursor += 1;
    }
    if body.get(cursor) != Some(&'=') {
        return None;
    }
    cursor += 1;
    while matches!(body.get(cursor), Some(c) if c.is_whitespace()) {
        cursor += 1;
    }
    let quote = *body.get(cursor)?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    cursor += 1;
    let id_chars: Vec<char> = id.chars().collect();
    for expected in id_chars {
        if body.get(cursor) != Some(&expected) {
            return None;
        }
        cursor += 1;
    }
    if body.get(cursor) != Some(&quote) {
        return None;
    }
    cursor += 1;
    while matches!(body.get(cursor), Some(c) if c.is_whitespace()) {
        cursor += 1;
    }
    if body.get(cursor) != Some(&']') {
        return None;
    }
    cursor += 1;
    let _ = chars;
    Some(cursor)
}

/// Collects elements for the helper modules (kept pub(crate) for tests).
pub fn elements_ref(document: &Document) -> Vec<&Element> {
    document_elements_ref(document)
}
