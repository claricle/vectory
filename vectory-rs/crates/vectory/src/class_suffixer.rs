//! Port of lib/vectory/svg_class_suffixer.rb — suffixes SVG class lists and
//! CSS class selectors without normalizing CSS.
//!
//! Semantics carried over exactly:
//! * XML `class` lists: every `[^\t\n\f\r ]+` token gets the suffix unless
//!   it already ends with it;
//! * `<style>` blocks: the complete CSS stream (all text nodes joined,
//!   across CDATA/comment boundaries) is tokenized crass-style; class
//!   selectors in qualified-rule preludes, nested rules, and the supported
//!   at-rules get insertions at exact token positions; insertions are then
//!   applied to the ORIGINAL text nodes so comments, PIs and CDATA stay in
//!   place;
//! * a trailing short hex escape that would absorb the first suffix
//!   character gets a whitespace terminator inserted before the suffix
//!   (escaped-identifier parity with the crass tokenizer, [`crate::css`]).

use std::collections::BTreeMap;

use crate::css::{parse_rules, tokenize, Rule, Token, TokenKind};
use crate::dom::{Document, Element, Node};
use crate::error::Error;

const RULE_AT_RULES: [&str; 10] = [
    "media",
    "supports",
    "layer",
    "container",
    "document",
    "-moz-document",
    "scope",
    "starting-style",
    "keyframes",
    "-webkit-keyframes",
];

const SELECTOR_FUNCTIONS: [&str; 9] = [
    "is",
    "where",
    "not",
    "has",
    "nth-child",
    "nth-last-child",
    "host",
    "host-context",
    "slotted",
];

/// Rewrites `content` (SVG XML, or a host document containing SVGs),
/// suffixing every class list and CSS class selector with `suffix`.
///
/// # Errors
///
/// Returns [`Error`] when the XML is malformed.
pub fn rewrite(content: &str, suffix: &str) -> Result<String, Error> {
    let mut document = Document::parse(content)?;
    let paths = svg_element_paths(&document);
    for path in &paths {
        // The svg root itself plus every descendant (`[svg] + svg.xpath(".//*")`).
        let mut targets = vec![path.clone()];
        {
            let root = document.root.as_ref().expect("parsed document has a root");
            if let Some(subtree) = element_at(root, path) {
                collect_descendant_paths(subtree, path, &mut targets);
            }
        }
        for target in &targets {
            if let Some(element) = element_at_mut(&mut document, target) {
                rewrite_element(element, suffix);
            }
        }
    }
    Ok(crate::serialize::serialize_document(&document))
}

/// Paths (child-index chains from the document root) of every `svg` element
/// in the document — `//*[local-name()='svg']`, root included.
fn svg_element_paths(document: &Document) -> Vec<Vec<usize>> {
    let mut paths = Vec::new();
    if let Some(root) = &document.root {
        let mut current = Vec::new();
        collect_svg_paths(root, &mut current, &mut paths);
    }
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

/// Absolute paths of every element under (and excluding) `subtree`.
fn collect_descendant_paths(subtree: &Element, base: &[usize], out: &mut Vec<Vec<usize>>) {
    for (index, child) in subtree.children.iter().enumerate() {
        if let Node::Element(child_element) = child {
            let mut path = base.to_vec();
            path.push(index);
            out.push(path.clone());
            collect_descendant_paths(child_element, &path, out);
        }
    }
}

fn rewrite_element(element: &mut Element, suffix: &str) {
    rewrite_classes(element, suffix);
    if element.name == "style" {
        rewrite_style(element, suffix);
    }
}

fn rewrite_classes(element: &mut Element, suffix: &str) {
    let Some(index) = element.attributes.iter().position(|a| a.name == "class") else {
        return;
    };
    let value = &mut element.attributes[index].value;
    // gsub(/[^\t\n\f\r ]+/) — rewrite each token in place, separators kept.
    let mut rewritten = String::with_capacity(value.len());
    let mut token = String::new();
    for c in value.chars() {
        if matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' ') {
            if !token.is_empty() {
                push_class_token(&mut rewritten, &token, suffix);
                token.clear();
            }
            rewritten.push(c);
        } else {
            token.push(c);
        }
    }
    if !token.is_empty() {
        push_class_token(&mut rewritten, &token, suffix);
    }
    *value = rewritten;
}

fn push_class_token(out: &mut String, token: &str, suffix: &str) {
    if token.ends_with(suffix) {
        out.push_str(token);
    } else {
        out.push_str(token);
        out.push_str(suffix);
    }
}

/// A text-bearing child of a `<style>` element, with its start offset in
/// the joined CSS stream.
struct Piece {
    start: usize,
    text: String,
}

fn rewrite_style(element: &mut Element, suffix: &str) {
    // Parse the complete CSS stream, even when an XML comment or CDATA
    // boundary splits a token. Apply insertions to the original text nodes
    // so comments, processing instructions and CDATA remain in place.
    let mut pieces: Vec<Piece> = Vec::new();
    let mut css = String::new();
    for child in &element.children {
        let text = match child {
            Node::Text(text) => text.0.clone(),
            Node::CData(content) => content.clone(),
            _ => continue,
        };
        css.push_str(&text);
        pieces.push(Piece {
            start: css.chars().count() - text.chars().count(),
            text,
        });
    }
    if pieces.is_empty() {
        return;
    }
    let changes = css_insertions(&css, suffix);
    // Apply in descending order; each position lands in exactly one piece
    // (the pieces tile the stream).
    let mut piece_texts: Vec<Vec<char>> = pieces
        .iter()
        .map(|piece| piece.text.chars().collect())
        .collect();
    for &(position, ref insertion) in changes.iter().rev() {
        for (index, piece) in pieces.iter().enumerate() {
            let length = piece.text.chars().count();
            if position > piece.start && position <= piece.start + length {
                let insert_at = position - piece.start;
                for (offset, c) in insertion.chars().enumerate() {
                    piece_texts[index].insert(insert_at + offset, c);
                }
                break;
            }
        }
    }
    // Write back, skipping unchanged pieces (`node.content = rewritten
    // unless rewritten == text`).
    let mut piece_iter = piece_texts.into_iter();
    for child in element.children.iter_mut() {
        let text = match child {
            Node::Text(text) => &mut text.0,
            Node::CData(content) => content,
            _ => continue,
        };
        if let Some(chars) = piece_iter.next() {
            let rewritten_text: String = chars.into_iter().collect();
            if rewritten_text != *text {
                *text = rewritten_text;
            }
        }
    }
}

/// Computes the char-position insertions for a CSS stream, mapped back to
/// the ORIGINAL source (token positions use CSS-normalized newlines; the
/// offsets table maps them back, exactly as the Ruby layer does).
fn css_insertions(css: &str, suffix: &str) -> Vec<(usize, String)> {
    let mut changes: BTreeMap<usize, String> = BTreeMap::new();
    rewrite_rules(&parse_rules(css), suffix, &mut changes);
    let offsets = source_offsets(css);
    changes
        .into_iter()
        .map(|(position, insertion)| (offsets[position], insertion))
        .collect()
}

/// Unit table: `offsets[i]` is the original-string char offset of the i-th
/// CSS-normalized position (a CRLF in the source counts as one unit but
/// occupies two chars — exactly the Ruby `scan(/\r\n|./m)` arithmetic).
fn source_offsets(css: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    let chars: Vec<char> = css.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let width = if chars[index] == '\r' && chars.get(index + 1) == Some(&'\n') {
            2
        } else {
            1
        };
        offsets.push(offsets.last().unwrap() + width);
        index += width;
    }
    offsets
}

fn rewrite_rules(rules: &[Rule], suffix: &str, changes: &mut BTreeMap<usize, String>) {
    for rule in rules {
        match rule {
            Rule::Qualified { prelude, block } => {
                rewrite_selector(prelude, suffix, changes);
                rewrite_nested_rules(block, suffix, changes);
            }
            Rule::AtRule {
                name,
                prelude,
                block,
            } => {
                rewrite_at_rule(name, prelude, block.as_deref(), suffix, changes);
            }
        }
    }
}

fn rewrite_at_rule(
    name: &str,
    prelude: &[Token],
    block: Option<&[Token]>,
    suffix: &str,
    changes: &mut BTreeMap<usize, String>,
) {
    let lowered = name.to_lowercase();
    let Some(block) = block else {
        return;
    };
    if !RULE_AT_RULES.contains(&lowered.as_str()) {
        return;
    }
    if lowered == "scope" {
        rewrite_scope(prelude, suffix, changes);
    }
    rewrite_nested_rules(block, suffix, changes);
}

fn rewrite_nested_rules(tokens: &[Token], suffix: &str, changes: &mut BTreeMap<usize, String>) {
    // crass's declaration parser predates CSS nesting and discards nested
    // rules; walk the component values keeping declaration values opaque.
    let mut prelude: Vec<Token> = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].clone();
        index += 1;
        match token.kind {
            TokenKind::Semicolon => prelude.clear(),
            TokenKind::AtKeyword => {
                if prelude.iter().all(Token::is_trivia) {
                    // Reconsume at the at-keyword: consume the whole at-rule.
                    let (name, at_prelude, block) = consume_at_rule(&token, tokens, &mut index);
                    rewrite_at_rule(&name, &at_prelude, block.as_deref(), suffix, changes);
                    prelude.clear();
                } else {
                    prelude.push(token);
                }
            }
            TokenKind::SimpleBlock if token.start == Some('{') && !custom_property(&prelude) => {
                rewrite_selector(&prelude, suffix, changes);
                rewrite_nested_rules(&token.tokens, suffix, changes);
                prelude.clear();
            }
            _ => prelude.push(token),
        }
    }
}

/// Consumes an at-rule from `index` (which sits after the at-keyword token
/// `at_keyword`): prelude up to the block or semicolon.
fn consume_at_rule(
    at_keyword: &Token,
    tokens: &[Token],
    index: &mut usize,
) -> (String, Vec<Token>, Option<Vec<Token>>) {
    let mut prelude = Vec::new();
    let mut block = None;
    while *index < tokens.len() {
        let token = tokens[*index].clone();
        *index += 1;
        match token.kind {
            TokenKind::Semicolon => break,
            TokenKind::SimpleBlock if token.start == Some('{') => {
                block = Some(token.tokens);
                break;
            }
            _ => prelude.push(token),
        }
    }
    (at_keyword.name.clone().unwrap_or_default(), prelude, block)
}

fn custom_property(tokens: &[Token]) -> bool {
    let significant: Vec<&Token> = tokens.iter().filter(|token| !token.is_trivia()).collect();
    matches!(
        (significant.first(), significant.get(1)),
        (Some(first), Some(second))
            if first.kind == TokenKind::Ident
                && first.value.starts_with("--")
                && second.kind == TokenKind::Colon
    )
}

fn rewrite_scope(tokens: &[Token], suffix: &str, changes: &mut BTreeMap<usize, String>) {
    for token in tokens {
        if token.kind == TokenKind::SimpleBlock && token.start == Some('(') {
            rewrite_selector(&token.tokens, suffix, changes);
        }
    }
}

fn rewrite_selector(tokens: &[Token], suffix: &str, changes: &mut BTreeMap<usize, String>) {
    let mut previous: Option<&Token> = None;
    for token in tokens {
        if token.kind == TokenKind::Comment {
            continue;
        }
        if class_identifier(token, previous) {
            suffix_identifier(token, suffix, changes);
        } else if selector_function(token) {
            rewrite_selector(&token.tokens, suffix, changes);
        }
        previous = Some(token);
    }
}

fn class_identifier(token: &Token, previous: Option<&Token>) -> bool {
    token.kind == TokenKind::Ident
        && previous
            .is_some_and(|previous| previous.kind == TokenKind::Delim && previous.value == ".")
}

fn selector_function(token: &Token) -> bool {
    token.kind == TokenKind::Function
        && token
            .name
            .as_deref()
            .is_some_and(|name| SELECTOR_FUNCTIONS.contains(&name.to_lowercase().as_str()))
}

fn suffix_identifier(token: &Token, suffix: &str, changes: &mut BTreeMap<usize, String>) {
    if token.value.ends_with(suffix) {
        return;
    }
    // A trailing short hex escape can absorb the first suffix character;
    // add its optional terminator only when concatenation changes meaning.
    let combined = tokenize(&format!("{}{}", token.raw, suffix));
    let insertion = match combined.first() {
        Some(first) if first.value == format!("{}{}", token.value, suffix) => suffix.to_string(),
        _ => format!(" {suffix}"),
    };
    changes.insert(token.pos + token.raw.chars().count(), insertion);
}
