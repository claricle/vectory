//! Port of lib/vectory/svg_mapping.rb — processes SVG mapping in XML
//! documents: extracting SVG from image tags or inline `<svg>` elements,
//! processing `<target>` elements to build link mappings, applying ID
//! suffixes, simplifying svgmap structure, and writing the result back into
//! the CALLER's document (the #84 contract).
//!
//! The caller-document write-back contract, on the model: the mapping owns
//! the caller's [`Document`] and `call` updates it in place — exactly the
//! regression class of issue #84, where processing never reached the
//! caller's tree and svgmap hotspots kept their unremapped hrefs.
//!
//! Deviations from the Ruby layer (documented, semantics-preserving for the
//! gated specs):
//! * file paths resolve against an explicit `base_dir` (the Ruby layer
//!   uses the process working directory); href expansion uses the same
//!   base on both sides of the links map, so keys match as in Ruby;
//! * the namespace-qualified XPath traversal is expressed as
//!   namespace-aware element predicates over index paths;
//! * non-SVG data URI payloads (EMF/PostScript) are rejected here — their
//!   models live in their own crates per the model-driven law.

use std::path::Path;

use crate::datauri;
use crate::dom::{Document, Element, Node};
use crate::error::Error;
use crate::mutation;
use crate::namespace::SVG_NAMESPACE_URI;
use crate::svg_document::SvgDocument;

/// Processes SVG mapping in XML documents.
pub struct SvgMapping {
    /// The caller's document when driven under the write-back contract.
    caller_doc: Option<Document>,
    doc: Document,
    base_dir: String,
    id_suffix: Option<String>,
}

impl SvgMapping {
    /// Builds a mapping over an XML string (no caller document).
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed XML.
    pub fn from_xml(xml: &str, base_dir: &Path, id_suffix: Option<&str>) -> Result<Self, Error> {
        Ok(SvgMapping {
            caller_doc: None,
            doc: Document::parse(xml)?,
            base_dir: base_dir.to_string_lossy().to_string(),
            id_suffix: id_suffix.map(str::to_string),
        })
    }

    /// Builds a mapping over a caller-owned document — the svgmap_cleanup
    /// contract: `call` writes the processed tree back into the caller.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed XML.
    pub fn from_caller_document(
        caller: Document,
        base_dir: &Path,
        id_suffix: Option<&str>,
    ) -> Result<Self, Error> {
        let xml = crate::serialize::serialize_document(&caller);
        Ok(SvgMapping {
            caller_doc: Some(caller),
            doc: Document::parse(&xml)?,
            base_dir: base_dir.to_string_lossy().to_string(),
            id_suffix: id_suffix.map(str::to_string),
        })
    }

    /// Processes all svgmap elements (in document order, index-suffixed)
    /// and writes back into the caller's document when one was supplied.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed SVG content.
    pub fn call(&mut self) -> Result<&Document, Error> {
        let svgmaps = self.svgmap_paths();
        for (index, path) in svgmaps.iter().enumerate() {
            self.process_svgmap(path, index)?;
        }
        if self.caller_doc.is_some() {
            self.write_back();
        }
        Ok(&self.doc)
    }

    /// Processes and returns the XML string (the Ruby `to_xml`).
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed SVG content.
    pub fn to_xml(&mut self) -> Result<String, Error> {
        self.call()?;
        Ok(crate::serialize::serialize_document(&self.doc))
    }

    /// The caller document, after `call` wrote the processed root children
    /// back into it — the regression assertion surface for #84.
    #[must_use]
    pub fn caller_document(&self) -> Option<&Document> {
        self.caller_doc.as_ref()
    }

    /// Moves the processed document's root children under the caller
    /// document's root (`root.children.each(&:remove)` + append — the same
    /// graft metanorma-standoc applies when it adopts the return-value
    /// contract).
    pub fn write_back(&mut self) {
        let Some(caller) = self.caller_doc.as_mut() else {
            return;
        };
        let children = self
            .doc
            .root
            .as_ref()
            .map(|root| root.children.clone())
            .unwrap_or_default();
        mutation::replace_root_children(caller, children);
    }

    fn root_namespace(&self) -> Option<String> {
        self.doc.root.as_ref().and_then(|root| root.namespace_uri.clone())
    }

    /// Paths (child-index chains) of the svgmap elements, document order,
    /// namespace-qualified against the root's namespace.
    fn svgmap_paths(&self) -> Vec<Vec<usize>> {
        let Some(root) = &self.doc.root else {
            return Vec::new();
        };
        let root_ns = root.namespace_uri.clone();
        let mut paths = Vec::new();
        let mut current = Vec::new();
        collect_named_paths(root, root_ns.as_deref(), "svgmap", &mut current, &mut paths);
        paths
    }

    fn process_svgmap(&mut self, svgmap_path: &[usize], index: usize) -> Result<(), Error> {
        // extract_image_tag: the FIRST image under the svgmap when its src
        // is non-empty, else the first svg element in the SVG namespace.
        let Some(image_path) = self.extract_image_tag(svgmap_path) else {
            return Ok(());
        };

        // Immutable analyses before any mutation (the Ruby layer mutates
        // its own tree in between; the results are order-independent).
        let links_map = self.targets_links_map(svgmap_path);
        let target_removals = self.target_removal_paths(svgmap_path);
        let content = self.generate_content(&image_path, index, &links_map)?;
        let Some(content) = content else {
            return Ok(());
        };

        // target_tag.remove — reverse sibling order keeps paths valid.
        for removal in target_removals.into_iter().rev() {
            self.remove_child(&removal);
        }

        // image.replace(content).
        self.replace_element_with_content(&image_path, &content)?;

        // simplify_svgmap.
        self.simplify_svgmap(svgmap_path);
        Ok(())
    }

    fn extract_image_tag(&self, svgmap_path: &[usize]) -> Option<Vec<usize>> {
        let root = self.doc.root.as_ref()?;
        let svgmap = element_at(root, svgmap_path)?;
        let root_ns = self.root_namespace();
        // First image element anywhere under the svgmap (depth-first).
        let image = {
            let mut found: Option<Vec<usize>> = None;
            let mut current = svgmap_path.to_vec();
            find_first_named(svgmap, svgmap_path, root_ns.as_deref(), "image", &mut current, &mut found);
            found
        };
        if let Some(image_path) = &image {
            let src = element_at(root, image_path)
                .and_then(|element| element.attribute("src"))
                .unwrap_or_default();
            if !src.is_empty() {
                return Some(image_path.clone());
            }
        }
        // First svg element in the SVG namespace.
        let mut found: Option<Vec<usize>> = None;
        let mut current = svgmap_path.to_vec();
        find_first_svg_ns(svgmap, svgmap_path, &mut current, &mut found);
        found
    }

    /// The `(expanded href, target)` pairs for resolvable targets.
    fn targets_links_map(&self, svgmap_path: &[usize]) -> Vec<(String, String)> {
        let mut map = Vec::new();
        for (target_path, target_value, href) in self.resolvable_targets(svgmap_path) {
            let _ = target_path;
            map.push((expand_path(&href, &self.base_dir), target_value));
        }
        map
    }

    /// Paths of the target children with a resolvable link target (they are
    /// removed from the svgmap).
    fn target_removal_paths(&self, svgmap_path: &[usize]) -> Vec<Vec<usize>> {
        self.resolvable_targets(svgmap_path)
            .into_iter()
            .map(|(path, _, _)| path)
            .collect()
    }

    fn resolvable_targets(&self, svgmap_path: &[usize]) -> Vec<(Vec<usize>, String, String)> {
        let Some(root) = self.doc.root.as_ref() else {
            return Vec::new();
        };
        let Some(svgmap) = element_at(root, svgmap_path) else {
            return Vec::new();
        };
        let root_ns = self.root_namespace();
        let mut out = Vec::new();
        for (index, child) in svgmap.children.iter().enumerate() {
            let Node::Element(child_element) = child else {
                continue;
            };
            if !mutation::is_named(child_element, root_ns.as_deref(), "target") {
                continue;
            }
            let Some(target) = link_target(child_element, root_ns.as_deref()) else {
                continue;
            };
            let href = child_element.attribute("href").unwrap_or_default().to_string();
            out.push((join_path(svgmap_path, index), target, href));
        }
        out
    }

    fn generate_content(
        &self,
        image_path: &[usize],
        index: usize,
        links_map: &[(String, String)],
    ) -> Result<Option<String>, Error> {
        let root = self.doc.root.as_ref().expect("mapping has a root");
        let Some(image) = element_at(root, image_path) else {
            return Ok(None);
        };
        let Some(svg_content) = self.build_svg_content(image)? else {
            return Ok(None);
        };
        let mut svg_document = SvgDocument::new(&svg_content)?;
        svg_document.namespace(index, links_map, self.base_dir.as_str(), self.id_suffix.as_deref())?;
        Ok(Some(svg_document.content()))
    }

    /// The SVG content for an image element: inline svg, data URI, or file.
    fn build_svg_content(&self, image: &Element) -> Result<Option<String>, Error> {
        if image.local_name() == "svg" && image.namespace_uri.as_deref() == Some(SVG_NAMESPACE_URI)
        {
            return Ok(Some(serialize_element(image)));
        }
        if image.local_name() != "image" {
            return Ok(None);
        }
        let src = image.attribute("src").unwrap_or_default();
        if src.starts_with("data:") {
            return Ok(Some(datauri::svg_content(src)?));
        }
        let path = expand_path(src, &self.base_dir);
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            Err(_) => Ok(None),
        }
    }

    /// Removes svgmap wrapper if no more target elements with eref remain.
    fn simplify_svgmap(&mut self, svgmap_path: &[usize]) {
        let root_ns = self.root_namespace();
        let has_eref = {
            let root = self.doc.root.as_ref().expect("mapping has a root");
            element_at(root, svgmap_path).is_some_and(|svgmap| {
                svgmap.children.iter().any(|child| {
                    matches!(child, Node::Element(target)
                        if mutation::is_named(target, root_ns.as_deref(), "target")
                            && target.children.iter().any(|grandchild| {
                                matches!(grandchild, Node::Element(eref)
                                    if mutation::is_named(eref, root_ns.as_deref(), "eref"))
                            }))
                })
            })
        };
        if has_eref {
            return;
        }
        let root = self.doc.root.as_mut().expect("mapping has a root");
        let figure_index = element_at(root, svgmap_path).and_then(|svgmap| {
            svgmap.children.iter().position(|child| {
                matches!(child, Node::Element(child_element)
                    if mutation::is_named(child_element, root_ns.as_deref(), "figure"))
            })
        });
        let Some(figure_index) = figure_index else {
            return;
        };
        let figure_path = join_path(svgmap_path, figure_index);
        let figure = element_at(root, &figure_path).cloned();
        let Some(figure) = figure else {
            return;
        };
        // svgmap.replace(figure)
        let (parent_path, index) = split_path(svgmap_path);
        let Some(parent) = element_at_mut(root, &parent_path) else {
            return;
        };
        if index < parent.children.len() {
            parent.children[index] = Node::Element(figure);
        }
    }

    fn remove_child(&mut self, path: &[usize]) {
        let (parent_path, index) = split_path(path);
        let Some(root) = self.doc.root.as_mut() else {
            return;
        };
        if let Some(parent) = element_at_mut(root, &parent_path) {
            if index < parent.children.len() {
                parent.children.remove(index);
            }
        }
    }

    fn replace_element_with_content(&mut self, path: &[usize], content: &str) -> Result<(), Error> {
        let (parent_path, index) = split_path(path);
        let replacement = match Document::parse(content) {
            Ok(processed) => processed
                .root
                .map(|root| vec![Node::Element(root)])
                .unwrap_or_default(),
            Err(error) => return Err(error),
        };
        let Some(root) = self.doc.root.as_mut() else {
            return Ok(());
        };
        let Some(parent) = element_at_mut(root, &parent_path) else {
            return Ok(());
        };
        if index < parent.children.len() {
            parent.children.remove(index);
            for (offset, node) in replacement.into_iter().enumerate() {
                parent.children.insert(index + offset, node);
            }
        }
        Ok(())
    }
}

fn split_path(path: &[usize]) -> (Vec<usize>, usize) {
    let index = *path.last().expect("path is non-empty");
    (path[..path.len() - 1].to_vec(), index)
}

/// The mapping-side path/key expansion (both sides of the links map use
/// this, so keys match; the Ruby layer expands against the process CWD,
/// which is the base directory of its callers).
#[must_use]
pub fn expand_path(value: &str, base: &str) -> String {
    if value.starts_with('/') {
        value.to_string()
    } else {
        let base = base.trim_end_matches('/');
        if base.is_empty() || base == "." {
            value.to_string()
        } else {
            format!("{base}/{value}")
        }
    }
}

fn join_path(path: &[usize], index: usize) -> Vec<usize> {
    let mut joined = path.to_vec();
    joined.push(index);
    joined
}

fn element_at<'a>(root: &'a Element, path: &[usize]) -> Option<&'a Element> {
    let mut cursor = root;
    for &index in path {
        match cursor.children.get(index) {
            Some(Node::Element(child)) => cursor = child,
            _ => return None,
        }
    }
    Some(cursor)
}

fn element_at_mut<'a>(root: &'a mut Element, path: &[usize]) -> Option<&'a mut Element> {
    let mut cursor = root;
    for &index in path {
        match cursor.children.get_mut(index) {
            Some(Node::Element(child)) => cursor = child,
            _ => return None,
        }
    }
    Some(cursor)
}

fn find_first_named(
    element: &Element,
    base: &[usize],
    namespace_uri: Option<&str>,
    local: &str,
    current: &mut Vec<usize>,
    found: &mut Option<Vec<usize>>,
) {
    if found.is_some() {
        return;
    }
    for (index, child) in element.children.iter().enumerate() {
        let Node::Element(child_element) = child else {
            continue;
        };
        current.push(index);
        if mutation::is_named(child_element, namespace_uri, local) {
            let mut path = base.to_vec();
            path.extend_from_slice(&current[base.len()..]);
            *found = Some(path);
            current.pop();
            return;
        }
        find_first_named(child_element, base, namespace_uri, local, current, found);
        current.pop();
        if found.is_some() {
            return;
        }
    }
}

fn find_first_svg_ns(
    element: &Element,
    base: &[usize],
    current: &mut Vec<usize>,
    found: &mut Option<Vec<usize>>,
) {
    if found.is_some() {
        return;
    }
    for (index, child) in element.children.iter().enumerate() {
        let Node::Element(child_element) = child else {
            continue;
        };
        current.push(index);
        if child_element.namespace_uri.as_deref() == Some(SVG_NAMESPACE_URI)
            && child_element.local_name() == "svg"
        {
            let mut path = base.to_vec();
            path.extend_from_slice(&current[base.len()..]);
            *found = Some(path);
            current.pop();
            return;
        }
        find_first_svg_ns(child_element, base, current, found);
        current.pop();
        if found.is_some() {
            return;
        }
    }
}

fn collect_named_paths(
    element: &Element,
    namespace_uri: Option<&str>,
    local: &str,
    current: &mut Vec<usize>,
    paths: &mut Vec<Vec<usize>>,
) {
    for (index, child) in element.children.iter().enumerate() {
        let Node::Element(child_element) = child else {
            continue;
        };
        current.push(index);
        if mutation::is_named(child_element, namespace_uri, local) {
            paths.push(current.clone());
        }
        collect_named_paths(child_element, namespace_uri, local, current, paths);
        current.pop();
    }
}

/// The link target of a target element: `#xref-target` for xref children,
/// else the link child's target attribute.
fn link_target(target_tag: &Element, root_ns: Option<&str>) -> Option<String> {
    for child in &target_tag.children {
        let Node::Element(child_element) = child else {
            continue;
        };
        if mutation::is_named(child_element, root_ns, "xref") {
            return Some(format!(
                "#{}",
                child_element.attribute("target").unwrap_or_default()
            ));
        }
    }
    for child in &target_tag.children {
        let Node::Element(child_element) = child else {
            continue;
        };
        if mutation::is_named(child_element, root_ns, "link") {
            return child_element.attribute("target").map(str::to_string);
        }
    }
    None
}

fn serialize_element(element: &Element) -> String {
    crate::serialize::serialize(
        &Document {
            prolog: Vec::new(),
            root: Some(element.clone()),
            epilog: Vec::new(),
        },
        crate::serialize::Profile::RubyXml,
    )
}
