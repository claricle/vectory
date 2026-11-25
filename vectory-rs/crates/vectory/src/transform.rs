//! Port of lib/vectory/svg_transform.rb + svg_transform_pipeline.rb — an
//! immutable, named operation at a serialized XML boundary, and the
//! pipeline that owns ordering and scope checks.

use std::collections::HashMap;

use crate::error::Error;
use crate::graft;
use crate::id_disambiguator;
use crate::namespace_binding::{self, BindingRequest};

/// The profiles: the only executable step sequences (whole-document and
/// selected-SVG).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Graft,
    SvgCleanup,
    GraftThenSvgCleanup,
}

impl Profile {
    /// The canonical step names for the profile, in order.
    #[must_use]
    pub fn steps(self) -> &'static [StepName] {
        match self {
            Profile::Graft => &[StepName::GraftInto],
            Profile::SvgCleanup => &[StepName::Disambiguate, StepName::SuffixClasses, StepName::BindNamespaces],
            Profile::GraftThenSvgCleanup => &[
                StepName::GraftInto,
                StepName::Disambiguate,
                StepName::SuffixClasses,
                StepName::BindNamespaces,
            ],
        }
    }

    fn from_steps(names: &[StepName]) -> Option<Profile> {
        match names {
            [StepName::GraftInto] => Some(Profile::Graft),
            [StepName::Disambiguate, StepName::SuffixClasses, StepName::BindNamespaces] => {
                Some(Profile::SvgCleanup)
            }
            [
                StepName::GraftInto,
                StepName::Disambiguate,
                StepName::SuffixClasses,
                StepName::BindNamespaces,
            ] => Some(Profile::GraftThenSvgCleanup),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepName {
    GraftInto,
    Disambiguate,
    SuffixClasses,
    BindNamespaces,
}

impl StepName {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            StepName::GraftInto => "graft_into",
            StepName::Disambiguate => "disambiguate",
            StepName::SuffixClasses => "suffix_classes",
            StepName::BindNamespaces => "bind_namespaces",
        }
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Profile::Graft => "graft",
            Profile::SvgCleanup => "svg_cleanup",
            Profile::GraftThenSvgCleanup => "graft_then_svg_cleanup",
        };
        write!(f, "{name}")
    }
}

/// An immutable, named operation at a serialized XML boundary. Options are
/// owned snapshots — caller-side mutation after construction cannot change
/// a captured step (the Ruby deep-freeze contract, expressed through
/// ownership).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgTransform {
    GraftInto {
        host_content: String,
        svg_host: bool,
    },
    Disambiguate {
        index: usize,
        duplicate_ids: HashMap<String, usize>,
    },
    SuffixClasses {
        index: usize,
    },
    BindNamespaces {
        request: BindingRequest,
    },
}

impl SvgTransform {
    /// Captures a host document for the graft step, validating it up front.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the host XML is malformed or rootless.
    pub fn graft_into(host_content: &str) -> Result<Self, Error> {
        let document = Document::parse_strict(host_content)
            .map_err(|error| Error::Parsing(format!("invalid host_content XML: {error}")))?;
        let root = document
            .root
            .ok_or_else(|| Error::Parsing("host_content has no document root".to_string()))?;
        Ok(SvgTransform::GraftInto {
            host_content: host_content.to_string(),
            svg_host: root.name == "svg",
        })
    }

    /// Builds the disambiguation step.
    #[must_use]
    pub fn disambiguate(index: usize, duplicate_ids: HashMap<String, usize>) -> Self {
        SvgTransform::Disambiguate {
            index,
            duplicate_ids,
        }
    }

    /// Builds the class-suffixing step.
    #[must_use]
    pub fn suffix_classes(index: usize) -> Self {
        SvgTransform::SuffixClasses { index }
    }

    /// Builds the namespace-binding step.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transform`] for an invalid namespace request.
    pub fn bind_namespaces(
        bind: &[(&str, &str)],
        unbind: &[&str],
        remap: &[(&str, &str)],
    ) -> Result<Self, Error> {
        Ok(SvgTransform::BindNamespaces {
            request: BindingRequest::new(bind, unbind, remap)?,
        })
    }

    #[must_use]
    fn name(&self) -> StepName {
        match self {
            SvgTransform::GraftInto { .. } => StepName::GraftInto,
            SvgTransform::Disambiguate { .. } => StepName::Disambiguate,
            SvgTransform::SuffixClasses { .. } => StepName::SuffixClasses,
            SvgTransform::BindNamespaces { .. } => StepName::BindNamespaces,
        }
    }

    fn suffix(index: usize) -> String {
        format!("_inject_{index}")
    }

    /// Applies the step at the serialized boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on malformed input or invalid requests.
    pub fn apply(&self, content: &str) -> Result<String, Error> {
        match self {
            SvgTransform::GraftInto { host_content, .. } => {
                graft::replace_root_content(host_content, content)
            }
            SvgTransform::Disambiguate {
                index,
                duplicate_ids,
            } => id_disambiguator::rewrite(content, &Self::suffix(*index), duplicate_ids),
            SvgTransform::SuffixClasses { index } => {
                crate::class_suffixer::rewrite(content, &Self::suffix(*index))
            }
            SvgTransform::BindNamespaces { request } => {
                namespace_binding::rewrite(content, request)
            }
        }
    }
}

use crate::dom::Document;

impl Document {
    fn parse_strict(content: &str) -> Result<Document, Error> {
        Document::parse(content)
    }
}

/// The pipeline: only whole-document and selected-SVG profiles are
/// executable; sequence validation happens on construction (before any
/// input is parsed) and scope validation happens before any step runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgTransformPipeline {
    steps: Vec<SvgTransform>,
}

impl SvgTransformPipeline {
    /// Builds a pipeline, validating the step sequence.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transform`] unless the names are a valid prefix of
    /// one of the executable profiles.
    pub fn new(steps: Vec<SvgTransform>) -> Result<Self, Error> {
        if steps.is_empty() {
            return Err(Error::Transform(
                "invalid step sequence: expected SVG transforms".to_string(),
            ));
        }
        let names: Vec<StepName> = steps.iter().map(SvgTransform::name).collect();
        let valid = [Profile::Graft, Profile::SvgCleanup, Profile::GraftThenSvgCleanup]
            .iter()
            .any(|profile| {
                profile.steps().len() >= names.len()
                    && profile.steps()[..names.len()] == names[..]
            });
        if !valid {
            return Err(Error::Transform(format!(
                "invalid step sequence: {}",
                names
                    .iter()
                    .map(|name| name.name())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            )));
        }
        Ok(SvgTransformPipeline { steps })
    }

    /// Extends the pipeline with more steps, re-validating.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transform`] on an invalid combined sequence.
    pub fn then(mut self, additions: Vec<SvgTransform>) -> Result<Self, Error> {
        self.steps.extend(additions);
        SvgTransformPipeline::new(self.steps)
    }

    /// The executable profile when the sequence is complete.
    #[must_use]
    pub fn profile(&self) -> Option<Profile> {
        let names: Vec<StepName> = self.steps.iter().map(SvgTransform::name).collect();
        Profile::from_steps(&names)
    }

    /// The captured steps.
    #[must_use]
    pub fn steps(&self) -> &[SvgTransform] {
        &self.steps
    }

    /// Executes the pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transform`] for an incomplete profile or a scope
    /// mismatch, and [`Error::Parsing`] for malformed input — all before
    /// any step mutates the boundary.
    pub fn call(&self, content: &str) -> Result<String, Error> {
        let Some(profile) = self.profile() else {
            let names: Vec<StepName> = self.steps.iter().map(SvgTransform::name).collect();
            return Err(Error::Transform(format!(
                "incomplete profile: {}",
                names
                    .iter()
                    .map(|name| name.name())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            )));
        };
        self.validate_scope(content, profile)?;
        let mut output = content.to_string();
        for step in &self.steps {
            output = step.apply(&output)?;
        }
        if profile == Profile::GraftThenSvgCleanup {
            return Ok(self.finalize_graft(&output));
        }
        Ok(output)
    }

    fn validate_scope(&self, content: &str, profile: Profile) -> Result<(), Error> {
        match profile {
            Profile::SvgCleanup => {
                let root = Document::parse(content)
                    .map_err(|error| Error::Parsing(format!("invalid content XML: {error}")))?
                    .root;
                if root.is_some_and(|root| root.name == "svg") {
                    return Ok(());
                }
                Err(Error::Transform(
                    "execution-scope mismatch: svg_cleanup requires an SVG root".to_string(),
                ))
            }
            Profile::GraftThenSvgCleanup => {
                if matches!(
                    self.steps.first(),
                    Some(SvgTransform::GraftInto { svg_host: true, .. })
                ) {
                    return Ok(());
                }
                Err(Error::Transform(
                    "execution-scope mismatch: graft_then_svg_cleanup requires an SVG host root"
                        .to_string(),
                ))
            }
            Profile::Graft => Ok(()),
        }
    }

    /// Keeps the cleaned root, including its new attributes and bindings,
    /// inside the captured envelope.
    fn finalize_graft(&self, output: &str) -> String {
        let SvgTransform::GraftInto { host_content, .. } =
            self.steps.first().expect("graft_then profile leads with graft_into")
        else {
            unreachable!("graft_then profile leads with graft_into");
        };
        let Some(offset) = crate::scan::root_offset(host_content) else {
            return output.to_string();
        };
        let prefix = &host_content[..offset];
        let tail = crate::scan::trailing_envelope(host_content);
        let mut root = Document::parse(output)
            .expect("pipeline output reparses")
            .root
            .expect("parsed content has a root");
        remove_redundant_namespaces(&mut root);
        format!(
            "{prefix}{}{tail}",
            crate::serialize::serialize(
                &Document {
                    prolog: Vec::new(),
                    root: Some(root),
                    epilog: Vec::new(),
                },
                crate::serialize::Profile::RubyXml
            )
        )
    }
}

/// Bindings sealed onto grafted children that the (new) root now supplies
/// are removed from descendants — the Ruby `remove_redundant_namespaces`.
fn remove_redundant_namespaces(root: &mut crate::dom::Element) {
    let inherited: Vec<(String, String)> = {
        let mut out = Vec::new();
        for attribute in &root.attributes {
            if let Some((prefix, uri)) =
                crate::namespace::namespace_declaration(&attribute.name, &attribute.value)
            {
                let name = if prefix.is_empty() {
                    "xmlns".to_string()
                } else {
                    format!("xmlns:{prefix}")
                };
                if let Some(existing) = out.iter_mut().find(|(n, _)| *n == name) {
                    existing.1 = uri;
                } else {
                    out.push((name, uri));
                }
            }
        }
        out
    };
    for child in &mut root.children {
        if let crate::dom::Node::Element(child_element) = child {
            remove_redundant_in(child_element, &inherited);
        }
    }
}

fn remove_redundant_in(element: &mut crate::dom::Element, inherited: &[(String, String)]) {
    let own: Vec<(String, String)> = element
        .attributes
        .iter()
        .filter_map(|attribute| {
            crate::namespace::namespace_declaration(&attribute.name, &attribute.value)
                .map(|(prefix, uri)| {
                    (
                        if prefix.is_empty() {
                            "xmlns".to_string()
                        } else {
                            format!("xmlns:{prefix}")
                        },
                        uri,
                    )
                })
        })
        .collect();
    let redundant: Vec<String> = own
        .iter()
        .filter(|(name, uri)| {
            inherited
                .iter()
                .any(|(inherited_name, inherited_uri)| inherited_name == name && inherited_uri == uri)
        })
        .map(|(name, _)| name.clone())
        .collect();
    element
        .attributes
        .retain(|attribute| !redundant.contains(&attribute.name));
    // The inherited scope for deeper descendants adds this element's own
    // (now-redundant-free) declarations.
    let mut child_inherited = inherited.to_vec();
    for (name, uri) in &own {
        if !redundant.contains(name) {
            if let Some(existing) = child_inherited.iter_mut().find(|(n, _)| n == name) {
                existing.1 = uri.clone();
            } else {
                child_inherited.push((name.clone(), uri.clone()));
            }
        }
    }
    for child in &mut element.children {
        if let crate::dom::Node::Element(child_element) = child {
            remove_redundant_in(child_element, &child_inherited);
        }
    }
}
