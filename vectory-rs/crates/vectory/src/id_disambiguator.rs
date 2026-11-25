//! Port of lib/vectory/svg_id_disambiguator.rb — disambiguates element ids
//! across multiple SVG documents embedded in one output: ids that appear in
//! more than one SVG get a per-document suffix, and every reference form is
//! rewritten to match — `url(#id)` in presentation attributes and `<style>`
//! blocks, `href`/`xlink:href` fragment references, and the `id` attributes
//! themselves.

use std::collections::HashMap;

use crate::dom::{Document, Element, Node};
use crate::error::Error;

/// SVG elements whose ids are referenced through presentation attributes
/// and CSS rather than href: the tag maps to the attributes that carry the
/// reference (`None` = the tag name itself is the CSS property).
fn iri_tag_properties(tag: &str) -> Option<Option<&'static [&'static str]>> {
    match tag {
        "clipPath" => Some(Some(&["clip-path"])),
        "color-profile" | "cursor" | "filter" | "mask" => Some(None),
        "linearGradient" | "radialGradient" | "pattern" => Some(Some(&["fill", "stroke"])),
        "marker" => Some(Some(&["marker", "marker-end", "marker-mid", "marker-start"])),
        _ => None,
    }
}

/// Ids that appear more than once across the given SVG contents.
///
/// # Errors
///
/// Returns [`Error`] when a content is not well-formed XML.
pub fn duplicate_ids(contents: &[&str]) -> Result<HashMap<String, usize>, Error> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for content in contents {
        let document = Document::parse(content)?;
        for element in document.elements() {
            if let Some(id) = element.attribute("id") {
                *counts.entry(id.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts.retain(|_, count| *count >= 2);
    Ok(counts)
}

/// Suffixes every id in `duplicate_ids` with `suffix` inside `content`,
/// rewriting all reference forms to match. Ids absent from the map are
/// left untouched.
///
/// # Errors
///
/// Returns [`Error`] when the content is not well-formed XML.
pub fn rewrite(
    content: &str,
    suffix: &str,
    duplicate_ids: &HashMap<String, usize>,
) -> Result<String, Error> {
    let mut document = Document::parse(content)?;
    for path in svg_roots(&document) {
        if let Some(svg) = element_at_mut(&mut document, &path) {
            rewrite_svg(svg, suffix, duplicate_ids);
        }
    }
    Ok(crate::serialize::serialize_document(&document))
}

/// `svg_roots`: the root when it is `svg`, else every embedded `svg`
/// element (SVGs inside a host document).
fn svg_roots(document: &Document) -> Vec<Vec<usize>> {
    let mut paths = Vec::new();
    let Some(root) = &document.root else {
        return paths;
    };
    if root.name == "svg" {
        paths.push(Vec::new());
        return paths;
    }
    let mut current = Vec::new();
    collect_svg_paths(root, &mut current, &mut paths);
    paths
}

fn collect_svg_paths(element: &Element, current: &mut Vec<usize>, paths: &mut Vec<Vec<usize>>) {
    if element.local_name() == "svg" {
        paths.push(current.clone());
    }
    for (index, child) in element.children.iter().enumerate() {
        if let Node::Element(child_element) = child {
            current.push(index);
            collect_svg_paths(child_element, current, paths);
            current.pop();
        }
    }
}

fn element_at_mut<'a>(document: &'a mut Document, path: &[usize]) -> Option<&'a mut Element> {
    let mut cursor = document.root.as_mut()?;
    for &index in path {
        match cursor.children.get_mut(index) {
            Some(Node::Element(child)) => cursor = child,
            _ => return None,
        }
    }
    Some(cursor)
}

fn rewrite_svg(svg: &mut Element, suffix: &str, ids: &HashMap<String, usize>) {
    // id_elems: descendants with an id, plus the svg root itself when it
    // carries one — the referenceable elements.
    let mut id_tag_names: Vec<String> = Vec::new();
    let mut descendants: Vec<&Element> = Vec::new();
    crate::mutation::elements(svg, &mut descendants);
    for element in &descendants[1..] {
        if element.attribute("id").is_some() {
            collect_iri_tag_names(element, &mut id_tag_names);
        }
    }
    if svg.attribute("id").is_some() {
        collect_iri_tag_names(svg, &mut id_tag_names);
    }
    let iri_properties = iri_properties(&id_tag_names);

    // Every element under the svg root (the root included) gets its
    // reference forms rewritten.
    let all_paths = all_element_paths(svg);
    for path in all_paths {
        if let Some(element) = element_at_relative(svg, &path) {
            rewrite_element(element, &iri_properties, suffix, ids);
        }
    }
}

fn collect_iri_tag_names(element: &Element, out: &mut Vec<String>) {
    if iri_tag_properties(&element.name).is_some() && !out.contains(&element.name) {
        out.push(element.name.clone());
    }
}

/// The CSS-referenceable properties present among the id-bearing elements,
/// plus "style" when any exist (standoc svg_iri_properties).
fn iri_properties(tag_names: &[String]) -> Vec<String> {
    let mut properties: Vec<String> = Vec::new();
    for tag in tag_names {
        match iri_tag_properties(tag) {
            Some(Some(attributes)) => {
                for attribute in attributes {
                    if !properties.iter().any(|p| p == attribute) {
                        properties.push((*attribute).to_string());
                    }
                }
            }
            Some(None) => {
                if !properties.iter().any(|p| p == tag) {
                    properties.push(tag.clone());
                }
            }
            None => {}
        }
    }
    if properties.is_empty() {
        return properties;
    }
    properties.push("style".to_string());
    properties
}

fn all_element_paths(root: &Element) -> Vec<Vec<usize>> {
    let mut paths = vec![Vec::new()];
    let mut current = Vec::new();
    collect_all_paths(root, &mut current, &mut paths);
    paths
}

fn collect_all_paths(element: &Element, current: &mut Vec<usize>, paths: &mut Vec<Vec<usize>>) {
    for (index, child) in element.children.iter().enumerate() {
        if let Node::Element(child_element) = child {
            current.push(index);
            paths.push(current.clone());
            collect_all_paths(child_element, current, paths);
            current.pop();
        }
    }
}

fn element_at_relative<'a>(root: &'a mut Element, path: &[usize]) -> Option<&'a mut Element> {
    let mut cursor = root;
    for &index in path {
        match cursor.children.get_mut(index) {
            Some(Node::Element(child)) => cursor = child,
            _ => return None,
        }
    }
    Some(cursor)
}

fn rewrite_element(
    element: &mut Element,
    iri_properties: &[String],
    suffix: &str,
    ids: &HashMap<String, usize>,
) {
    if element.name == "style" {
        let text: String = element
            .children
            .iter()
            .map(|child| match child {
                Node::Text(text) => text.0.clone(),
                Node::CData(content) => content.clone(),
                _ => String::new(),
            })
            .collect();
        element.children = vec![Node::Text(crate::dom::Text(rewrite_urls(
            &text, suffix, ids,
        )))];
        return;
    }

    for property in iri_properties {
        let Some(index) = element
            .attributes
            .iter()
            .position(|a| a.name == *property)
        else {
            continue;
        };
        let value = &mut element.attributes[index].value;
        *value = rewrite_urls(value, suffix, ids);
    }
    for attribute in &mut element.attributes {
        match attribute.name.as_str() {
            "id" => {
                if ids.contains_key(&attribute.value) {
                    attribute.value = format!("{}{}", attribute.value, suffix);
                }
            }
            "xlink:href" | "href" => {
                let trimmed = attribute.value.trim();
                if trimmed.starts_with('#')
                    && ids.contains_key(trimmed.strip_prefix('#').unwrap_or(""))
                {
                    attribute.value = format!("{}{}", attribute.value, suffix);
                }
            }
            _ => {}
        }
    }
}

/// `url("?#id"?)` references whose id is in the map get the suffix
/// (`url("?#id<suffix>"?)`).
fn rewrite_urls(text: &str, suffix: &str, ids: &HashMap<String, usize>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let is_url = chars[index] == 'u'
            && chars.get(index + 1) == Some(&'r')
            && chars.get(index + 2) == Some(&'l')
            && chars.get(index + 3) == Some(&'(');
        if !is_url {
            out.push(chars[index]);
            index += 1;
            continue;
        }
        let mut cursor = index + 4;
        let mut quoted = false;
        if chars.get(cursor) == Some(&'"') {
            quoted = true;
            cursor += 1;
        }
        let mut matched = false;
        if chars.get(cursor) == Some(&'#') {
            let id_start = cursor + 1;
            if chars
                .get(id_start)
                .is_some_and(|c| c.is_ascii_alphabetic())
            {
                let mut end = id_start + 1;
                while chars
                    .get(end)
                    .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'))
                {
                    end += 1;
                }
                let id: String = chars[id_start..end].iter().collect();
                let mut tail = end;
                if quoted && chars.get(tail) == Some(&'"') {
                    tail += 1;
                }
                if chars.get(tail) == Some(&')') {
                    matched = true;
                    cursor = tail + 1;
                    // Unmatched ids keep the original text (`x` unchanged);
                    // matched ids get the suffix inserted after the id.
                    out.extend(&chars[index..id_start]);
                    out.push_str(&id);
                    if ids.contains_key(&id) {
                        out.push_str(suffix);
                    }
                    out.extend(&chars[end..cursor]);
                    index = cursor;
                    continue;
                }
            }
        }
        if !matched {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}
