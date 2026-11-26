//! The clip-path registry: resolves `clip-path="url(#X)"` references and
//! plans the native clip scopes the translation emits (the sibling-carry,
//! selected, pending, and interval plans), mirroring the reference
//! `Svg::ClipPathRegistry` over the resolved tree.

use std::collections::HashMap;

use crate::elements::{Kind, PathKind, Tree};
use crate::paint::Paint;
use eikogram::Matrix;

/// The clip-path registry and its scope plans.
pub struct ClipPathRegistry {
    clip_paths: HashMap<String, usize>,
    references: HashMap<String, usize>,
    exports: HashMap<usize, usize>,
    imports: HashMap<usize, usize>,
    selected: HashMap<usize, usize>,
    unused_selected: HashMap<usize, ()>,
    interval_starts: HashMap<usize, usize>,
    interval_ends: HashMap<usize, usize>,
    interval_groups: HashMap<usize, ()>,
}

impl ClipPathRegistry {
    /// Builds the registry from the document tree (the planning passes run
    /// in the reference's order).
    #[must_use]
    pub fn from_tree(tree: &Tree) -> Self {
        let mut registry = ClipPathRegistry {
            clip_paths: HashMap::new(),
            references: HashMap::new(),
            exports: HashMap::new(),
            imports: HashMap::new(),
            selected: HashMap::new(),
            unused_selected: HashMap::new(),
            interval_starts: HashMap::new(),
            interval_ends: HashMap::new(),
            interval_groups: HashMap::new(),
        };
        collect(tree, tree.root, &mut registry);
        registry.plan_sibling_clips(tree, tree.root);
        registry.plan_selected_clips(tree, tree.root);
        let header = header_group(tree);
        registry.plan_clip_intervals(tree, header);
        registry.plan_text_clip_intervals(tree, tree.root);
        registry.plan_pending_clips(tree, tree.root, header);
        registry
    }

    /// Counts a clip reference from a resolved element's attribute.
    pub fn reference(&mut self, attribute: Option<&str>) {
        if let Some(id) = attribute.and_then(clip_reference_id) {
            *self.references.entry(id).or_insert(0) += 1;
        }
    }

    /// Whether a clip id was referenced anywhere.
    #[must_use]
    pub fn referenced(&self, id: &str) -> bool {
        self.references.contains_key(id)
    }

    /// The resolved clip element for an attribute.
    #[must_use]
    pub fn lookup(&self, attribute: Option<&str>) -> Option<usize> {
        let id = clip_reference_id(attribute?)?;
        self.clip_paths.get(&id).copied()
    }

    #[must_use]
    pub fn exported_clip(&self, group: usize) -> Option<usize> {
        self.exports.get(&group).copied()
    }

    #[must_use]
    pub fn imported_clip(&self, group: usize) -> Option<usize> {
        self.imports.get(&group).copied()
    }

    #[must_use]
    pub fn selected_clip(&self, group: usize) -> Option<usize> {
        self.selected.get(&group).copied()
    }

    #[must_use]
    pub fn unused_selected_clip(&self, clip: usize) -> bool {
        self.unused_selected.contains_key(&clip)
    }

    #[must_use]
    pub fn interval_start(&self, element: usize) -> Option<usize> {
        self.interval_starts.get(&element).copied()
    }

    #[must_use]
    pub fn interval_end(&self, element: usize) -> Option<usize> {
        self.interval_ends.get(&element).copied()
    }

    #[must_use]
    pub fn interval_group(&self, element: usize) -> bool {
        self.interval_groups.contains_key(&element)
    }

    /// Debug introspection for the parity iteration (not an API contract):
    /// the planned interval starts as (element, clip) pairs.
    #[must_use]
    pub fn interval_starts_debug(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = self
            .interval_starts
            .iter()
            .map(|(element, clip)| (*element, *clip))
            .collect();
        out.sort();
        out
    }

    /// Debug introspection: a clip's planned unused-rectangle bounds.
    #[must_use]
    pub fn unused_rectangle_debug(&self, tree: &Tree, clip: usize) -> Option<[f64; 4]> {
        unused_rectangle_bounds(tree, clip)
    }

    fn plan_selected_clips(&mut self, tree: &Tree, group: usize) {
        for child in tree.children(group).to_vec() {
            self.plan_selected_clips(tree, child);
        }
        if let Some(clip) = selected_group_clip(tree, self, group) {
            self.selected.insert(group, clip);
        }
    }

    fn plan_sibling_clips(&mut self, tree: &Tree, parent: usize) {
        for child in tree.children(parent).to_vec() {
            self.plan_sibling_clips(tree, child);
        }
        let Kind::Group {
            transform,
            canonical_matrix,
            ..
        } = tree.kind(parent)
        else {
            return;
        };
        if !*canonical_matrix || !matrix_identity(transform) {
            return;
        }
        let children = tree.children(parent).to_vec();
        for pair in children.windows(2) {
            if let Some(clip) = sibling_clip(tree, self, pair[0], pair[1]) {
                self.exports.insert(pair[0], clip);
                self.imports.insert(pair[1], clip);
            }
        }
    }

    fn plan_pending_clips(&mut self, tree: &Tree, parent: usize, header: Option<usize>) {
        for child in tree.children(parent).to_vec() {
            self.plan_pending_clips(tree, child, header);
        }
        let is_canonical_group = match tree.kind(parent) {
            Kind::Group {
                transform,
                canonical_matrix,
                ..
            } => *canonical_matrix && matrix_identity(transform),
            _ => false,
        };
        if Some(parent) != header && !is_canonical_group {
            return;
        }
        let children = tree.children(parent).to_vec();
        for index in 0..children.len().saturating_sub(1) {
            let (path, defs) = (children[index], children[index + 1]);
            let defs_children = tree.children(defs);
            if defs_children.len() != 1 {
                continue;
            }
            let clip = defs_children[0];
            if !matches!(tree.kind(clip), Kind::ClipPath { .. }) {
                continue;
            }
            if !matching_pending_clip(tree, path, clip) {
                continue;
            }
            let referenced = clip_id(tree, clip)
                .map(|id| self.referenced(&id))
                .unwrap_or(false);
            if referenced {
                self.plan_pending_draw_interval(tree, children[index + 2..].to_vec(), defs, clip);
            } else {
                self.unused_selected.insert(clip, ());
            }
        }
    }

    fn plan_pending_draw_interval(
        &mut self,
        tree: &Tree,
        children: Vec<usize>,
        defs: usize,
        clip: usize,
    ) {
        let mut draws = Vec::new();
        for draw in children {
            let supported = match tree.kind(draw) {
                Kind::Image { .. } => true,
                Kind::Path {
                    style,
                    path_record_kind,
                    ..
                } => {
                    style.stroke.is_some()
                        && !matches!(style.fill, Paint::Null)
                        && *path_record_kind != Some(PathKind::EndPath)
                }
                _ => false,
            };
            if !supported || self.lookup(tree.kind(draw).clip_path()) != Some(clip) {
                break;
            }
            draws.push(draw);
        }
        if draws.is_empty() {
            return;
        }
        let total = clip_id(tree, clip)
            .and_then(|id| self.references.get(&id).copied())
            .unwrap_or(0);
        if total != draws.len() {
            return;
        }
        self.interval_starts.insert(defs, clip);
        self.interval_ends.insert(draws[draws.len() - 1], clip);
    }

    fn plan_clip_intervals(&mut self, tree: &Tree, parent: Option<usize>) {
        let Some(parent) = parent else {
            return;
        };
        let mut canonical_groups = Vec::new();
        for child in tree.children(parent).to_vec() {
            if let Kind::Group {
                canonical_matrix, ..
            } = tree.kind(child)
            {
                if *canonical_matrix {
                    canonical_groups.push(child);
                }
            }
        }
        for group in canonical_groups {
            self.plan_clip_intervals(tree, Some(group));
        }
        let children = tree.children(parent).to_vec();
        // Group child positions by their resolved clip.
        let mut clips: Vec<usize> = Vec::new();
        let mut by_clip: HashMap<usize, Vec<usize>> = HashMap::new();
        for (index, child) in children.iter().enumerate() {
            let clip = self.lookup(tree.kind(*child).clip_path());
            let Some(clip) = clip else {
                continue;
            };
            if !by_clip.contains_key(&clip) {
                clips.push(clip);
            }
            by_clip.entry(clip).or_default().push(index);
        }
        let mut candidates: Vec<(usize, Vec<usize>)> = clips
            .into_iter()
            .map(|clip| (clip, by_clip[&clip].clone()))
            .filter(|(_, indices)| indices.len() >= 2)
            .collect();
        if std::env::var("EMF_DEBUG_INTERVALS").is_ok() {
            for (clip, indices) in &candidates {
                eprintln!(
                    "candidate clip{clip} n={} first={} last={} bounds={:?}",
                    indices.len(),
                    indices[0],
                    indices[indices.len() - 1],
                    unused_rectangle_bounds(tree, *clip)
                );
            }
        }
        candidates
            .sort_by_key(|(_, indices)| std::cmp::Reverse(indices[indices.len() - 1] - indices[0]));
        let mut occupied: Vec<(usize, usize)> = Vec::new();
        for (clip, indices) in candidates {
            let begin = indices[0];
            let end = indices[indices.len() - 1];
            let compatible = {
                let crossing = occupied
                    .iter()
                    .any(|(low, _high)| (*low <= begin) != (*low <= end));
                if crossing {
                    false
                } else {
                    let parents = occupied
                        .iter()
                        .filter(|(low, high)| *low <= begin && end <= *high)
                        .count();
                    let uniform = (begin..=end).all(|position| {
                        matches!(tree.kind(children[position]), Kind::Defs { .. })
                            || self.lookup(tree.kind(children[position]).clip_path()) == Some(clip)
                    });
                    if parents == 0 {
                        // The reference accepts a non-uniform outermost
                        // range (inner clips nest their own scopes) and
                        // only requires uniformity for nested ranges.
                        !uniform
                    } else {
                        parents == 1 && uniform
                    }
                }
            };
            if !compatible {
                continue;
            }
            if unused_rectangle_bounds(tree, clip).is_none() {
                continue;
            }
            let all_children = (begin..=end)
                .all(|position| self.interval_child(tree, children[position], clip));
            if !all_children {
                if std::env::var("EMF_DEBUG_INTERVALS").is_ok() {
                    let failing = (begin..=end)
                        .find(|position| {
                            !self.interval_child(tree, children[*position], clip)
                        })
                        .unwrap_or(begin);
                    eprintln!(
                        "clip{clip} rejected: interval_child fails at child#{failing} ({} clip={:?})",
                        tree.kind_name(children[failing]),
                        tree.clip_attr(children[failing])
                    );
                    if tree.kind_name(children[failing]) == "defs" {
                        for def_child in tree.children(children[failing]) {
                            eprintln!(
                                "    defs child: {}",
                                tree.kind_name(*def_child)
                            );
                        }
                    }
                }
                continue;
            }
            self.interval_starts.insert(children[begin], clip);
            self.interval_ends.insert(children[end], clip);
            occupied.push((begin, end));
            self.interval_groups.insert(parent, ());
        }
    }

    fn plan_text_clip_intervals(&mut self, tree: &Tree, parent: usize) {
        for child in tree.children(parent).to_vec() {
            self.plan_text_clip_intervals(tree, child);
        }
        let Kind::Group {
            canonical_matrix, ..
        } = tree.kind(parent)
        else {
            return;
        };
        if !*canonical_matrix {
            return;
        }
        let children = tree.children(parent).to_vec();
        if children.len() < 5 {
            return;
        }
        for window in children.windows(5) {
            let Some(clip) = self.text_interval_clip(tree, window) else {
                continue;
            };
            // The general rectangle planner may already start this same
            // interval at its first text; move that start to the
            // definition instead of opening two native scopes.
            if self.interval_starts.get(&window[2]) == Some(&clip) {
                self.interval_starts.remove(&window[2]);
            }
            self.interval_starts.insert(window[1], clip);
            self.interval_ends.insert(window[4], clip);
        }
    }

    fn text_interval_clip(&self, tree: &Tree, window: &[usize]) -> Option<usize> {
        let (previous, defs, first, path, last) =
            (window[0], window[1], window[2], window[3], window[4]);
        let children = tree.children(defs);
        if children.len() != 1 {
            return None;
        }
        let clip = children[0];
        if !matches!(tree.kind(clip), Kind::ClipPath { .. }) {
            return None;
        }
        let clip_id_value = clip_id(tree, clip)?;
        if self.references.get(&clip_id_value).copied() != Some(2) {
            return None;
        }
        for text in [first, last] {
            let Kind::Text {
                transform, style, ..
            } = tree.kind(text)
            else {
                return None;
            };
            if !matrix_identity(transform) {
                return None;
            }
            if self.lookup(style.clip_path.as_deref()) != Some(clip) {
                return None;
            }
        }
        let Kind::Path {
            style,
            path_record_kind,
            ..
        } = tree.kind(path)
        else {
            return None;
        };
        if style.clip_path.is_some() || *path_record_kind != Some(PathKind::StrokeAndFillPath) {
            return None;
        }
        if unused_rectangle_bounds(tree, clip).is_some() {
            return Some(clip);
        }
        matching_pending_clip(tree, previous, clip).then_some(clip)
    }

    fn interval_child(&self, tree: &Tree, child: usize, outer: usize) -> bool {
        match tree.kind(child) {
            Kind::Defs { unused_pattern, .. } => {
                // Creating an unselected canonical brush does not change
                // the active clip.
                if unused_pattern.is_some() {
                    return true;
                }
                tree.children(child).iter().all(|clip| {
                    matches!(tree.kind(*clip), Kind::ClipPath { .. })
                        && unused_rectangle_bounds(tree, *clip).is_some()
                        && clip_id(tree, *clip)
                            .map(|id| self.referenced(&id))
                            .unwrap_or(false)
                })
            }
            Kind::Path { style, .. } => {
                if style.clip_path.is_none()
                    && style
                        .stroke
                        .as_ref()
                        .is_some_and(|stroke| stroke.null_pen_fallback)
                {
                    return true;
                }
                self.bounds_within(tree, child, outer)
            }
            Kind::Rect { .. }
            | Kind::Ellipse { .. }
            | Kind::Circle { .. }
            | Kind::Line { .. }
            | Kind::Polyline { .. }
            | Kind::Polygon { .. }
            | Kind::Image { .. } => self.bounds_within(tree, child, outer),
            Kind::Text {
                transform, style, ..
            } => {
                if !matrix_identity(transform)
                    && self.lookup(style.clip_path.as_deref()) != Some(outer)
                {
                    return false;
                }
                self.bounds_within(tree, child, outer)
            }
            _ => false,
        }
    }

    fn bounds_within(&self, tree: &Tree, child: usize, outer: usize) -> bool {
        let Some(bounds) = self
            .lookup(tree.kind(child).clip_path())
            .and_then(|clip| unused_rectangle_bounds(tree, clip))
        else {
            return false;
        };
        let Some([left, top, right, bottom]) = unused_rectangle_bounds(tree, outer) else {
            return false;
        };
        bounds[0] >= left && bounds[1] >= top && bounds[2] <= right && bounds[3] <= bottom
    }
}

fn matrix_identity(matrix: &Matrix) -> bool {
    *matrix == Matrix::identity()
}

fn clip_reference_id(attribute: &str) -> Option<String> {
    let rest = attribute.trim().strip_prefix("url(")?;
    let rest = rest.strip_suffix(')')?;
    let rest = rest.trim().strip_prefix('#')?;
    if rest.contains(char::is_whitespace) {
        None
    } else {
        Some(rest.to_string())
    }
}

fn clip_id(tree: &Tree, node: usize) -> Option<String> {
    match tree.kind(node) {
        Kind::ClipPath {
            clip_id: Some(id), ..
        } => Some(id.clone()),
        _ => None,
    }
}

/// The bounds of the clip's region, for IntersectClipRect emission.
#[must_use]
pub fn unused_rectangle_bounds(tree: &Tree, clip: usize) -> Option<[f64; 4]> {
    match tree.kind(clip) {
        Kind::ClipPath {
            unused_rectangle_bounds,
            ..
        } => *unused_rectangle_bounds,
        _ => None,
    }
}

fn collect(tree: &Tree, node: usize, registry: &mut ClipPathRegistry) {
    let element = tree.elements[node].clone();
    if let Kind::ClipPath {
        clip_id: Some(id), ..
    } = &element.kind
    {
        registry.clip_paths.insert(id.clone(), node);
    }
    registry.reference(element.kind.clip_path());
    for child in &element.children {
        collect(tree, *child, registry);
    }
}

/// The document's header translate group (the root's first child group).
#[must_use]
pub fn header_group(tree: &Tree) -> Option<usize> {
    let root = tree.elements[tree.root].clone();
    let first = root.children.first().copied()?;
    matches!(tree.kind(first), Kind::Group { .. }).then_some(first)
}

fn selected_group_clip(tree: &Tree, registry: &ClipPathRegistry, group: usize) -> Option<usize> {
    let Kind::Group {
        transform,
        canonical_matrix,
        ..
    } = tree.kind(group)
    else {
        return None;
    };
    if !*canonical_matrix || !matrix_identity(transform) {
        return None;
    }
    let children = tree.children(group);
    if children.len() < 3 {
        return None;
    }
    let (unused, path, defs, draws) = (children[0], children[1], children[2], &children[3..]);
    if !unused_clip_definitions(tree, registry, unused) {
        return None;
    }
    let Kind::Path {
        path_record_kind,
        style,
        ..
    } = tree.kind(path)
    else {
        return None;
    };
    if *path_record_kind != Some(PathKind::EndPath) || style.clip_path.is_some() {
        return None;
    }
    let defs_children = tree.children(defs);
    if defs_children.len() != 1 || draws.is_empty() {
        return None;
    }
    let clip = defs_children[0];
    if !matches!(tree.kind(clip), Kind::ClipPath { .. }) {
        return None;
    }
    let Kind::ClipPath {
        selected_path_data: Some(selected),
        ..
    } = tree.kind(clip)
    else {
        return None;
    };
    let path_commands = match tree.kind(path) {
        Kind::Path { commands, .. } => commands,
        _ => return None,
    };
    if !matching_clip_path(selected, path_commands) {
        return None;
    }
    // Exclusive draws: every draw is a path referencing exactly this clip.
    let referenced = clip_id(tree, clip)
        .and_then(|id| registry.references.get(&id).copied())
        .unwrap_or(0);
    if referenced != draws.len() {
        return None;
    }
    draws
        .iter()
        .all(|draw| {
            matches!(tree.kind(*draw), Kind::Path { .. })
                && registry.lookup(tree.kind(*draw).clip_path()) == Some(clip)
        })
        .then_some(clip)
}

fn unused_clip_definitions(tree: &Tree, registry: &ClipPathRegistry, defs: usize) -> bool {
    let children = tree.children(defs);
    !children.is_empty()
        && children.iter().all(|child| {
            matches!(tree.kind(*child), Kind::ClipPath { .. })
                && unused_rectangle_bounds(tree, *child).is_some()
                && clip_id(tree, *child)
                    .map(|id| !registry.referenced(&id))
                    .unwrap_or(false)
        })
}

fn matching_pending_clip(tree: &Tree, path: usize, clip: usize) -> bool {
    let Kind::ClipPath {
        selected_path_data: Some(selected),
        ..
    } = tree.kind(clip)
    else {
        return false;
    };
    let Kind::Path {
        path_record_kind,
        style,
        commands,
        ..
    } = tree.kind(path)
    else {
        return false;
    };
    *path_record_kind == Some(PathKind::EndPath)
        && style.clip_path.is_none()
        && matching_clip_path(selected, commands)
}

fn matching_clip_path(selected: &str, path_commands: &[crate::path_data::Command]) -> bool {
    let Ok(parsed) = crate::path_data::parse(Some(selected)) else {
        return false;
    };
    crate::provenance::commands_match(path_commands, &parsed)
}

fn sibling_clip(
    tree: &Tree,
    registry: &ClipPathRegistry,
    source: usize,
    target: usize,
) -> Option<usize> {
    let Kind::Group {
        transform: source_transform,
        canonical_matrix: source_canonical,
        ..
    } = tree.kind(source)
    else {
        return None;
    };
    if !*source_canonical || !matrix_identity(source_transform) {
        return None;
    }
    let Kind::Group {
        canonical_matrix: target_canonical,
        ..
    } = tree.kind(target)
    else {
        return None;
    };
    if !*target_canonical {
        return None;
    }
    if tree.children(source).len() != 1 || tree.children(target).len() != 1 {
        return None;
    }
    let defs = tree.children(source)[0];
    let path = tree.children(target)[0];
    let defs_children = tree.children(defs);
    if defs_children.len() != 1 || !matches!(tree.kind(path), Kind::Path { .. }) {
        return None;
    }
    let clip = defs_children[0];
    if !matches!(tree.kind(clip), Kind::ClipPath { .. }) {
        return None;
    }
    unused_rectangle_bounds(tree, clip)?;
    let clip_id_value = clip_id(tree, clip)?;
    let referenced_once = registry.references.get(&clip_id_value).copied() == Some(1);
    if !referenced_once {
        return None;
    }
    if registry.lookup(tree.kind(path).clip_path()) != Some(clip) {
        return None;
    }
    Some(clip)
}
