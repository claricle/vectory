//! Shared helpers for the corpus and fidelity tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The repository's spec/ directory (the vectory corpus).
#[must_use]
pub fn repo_spec_dir() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../spec")).to_path_buf()
}

/// vectory-rs/testdata, where goldens and emfsvg renderer outputs live.
#[must_use]
pub fn testdata_dir() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).to_path_buf()
}

/// The golden bytes path for a corpus-relative path.
#[must_use]
pub fn golden_path(rel: &Path) -> PathBuf {
    testdata_dir().join("golden").join(rel)
}

/// Every `*.svg` under spec/, as (relative path, content), sorted.
#[must_use]
pub fn corpus_files() -> Vec<(PathBuf, String)> {
    let spec_root = repo_spec_dir();
    let mut out = Vec::new();
    let mut stack = vec![spec_root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "svg") {
                let rel = path.strip_prefix(&spec_root).expect("under spec root").to_path_buf();
                let content =
                    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel:?}: {e}"));
                out.push((rel, content));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Asserts that `source` fails to decode (the named-error corpus class).
pub fn decode_or_err(source: &str, rel: &Path) {
    match vectory::decode(source) {
        Ok(_) => panic!("{rel:?}: expected decode error"),
        Err(e) => assert!(
            matches!(e, vectory::Error::Xml(_) | vectory::Error::NoRoot),
            "{rel:?}: expected a named error, got {e}"
        ),
    }
}

/// Normalizes the renderer quirks that XML parsing cannot represent, so the
/// emfsvg-profile output can be compared byte-for-byte with the source:
///
/// * whitespace runs between attributes collapse to one space (the renderer
///   emits trailing spaces per attribute segment);
/// * whitespace before `=` is dropped (`style ="..."`);
/// * whitespace before `/>` is dropped (`"...none" />`);
/// * CDATA sections and comments are passed through verbatim.
///
/// Attribute values are never touched (quoted regions are verbatim), so
/// quirks that live INSIDE values — `translate(0.0000, 0.00 00)`, path `d`
/// trailing spaces — must round-trip exactly and are compared byte-for-byte.
#[must_use]
pub fn normalize_quirks(input: &str) -> String {
    let bytes = input;
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with("<![CDATA[") {
            let Some(end) = rest.find("]]>") else {
                out.push_str(rest);
                break;
            };
            let end = end + "]]>".len();
            out.push_str(&rest[..end]);
            i += end;
            continue;
        }
        if rest.starts_with("<!--") {
            let Some(end) = rest.find("-->") else {
                out.push_str(rest);
                break;
            };
            let end = end + "-->".len();
            out.push_str(&rest[..end]);
            i += end;
            continue;
        }
        let ch = rest.chars().next().expect("non-empty rest");
        if ch == '<' {
            let (consumed, normalized) = normalize_header(rest);
            out.push_str(&normalized);
            i += consumed;
            continue;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Normalizes one markup header (`<?...?>`, `</name>`, `<name ...>`,
/// `<name ... />`): whitespace outside quoted attribute values collapses to
/// a single space, disappears before `=`, `/>`, and `>`.
fn normalize_header(header: &str) -> (usize, String) {
    let mut out = String::new();
    let mut chars = header.char_indices();
    let mut in_quotes = false;
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '"' if in_quotes => {
                in_quotes = false;
                out.push(ch);
            }
            '"' => {
                in_quotes = true;
                out.push(ch);
            }
            '>' if !in_quotes => {
                out.push('>');
                return (idx + 1, out);
            }
            c if c.is_whitespace() && !in_quotes => {
                // Look ahead past the whitespace run.
                let after = &header[idx + c.len_utf8()..];
                let trimmed = after.trim_start();
                let skipped = after.len() - trimmed.len();
                for _ in 0..skipped {
                    chars.next();
                }
                let next = trimmed.chars().next();
                match next {
                    Some('=') | Some('>') => {}
                    Some('/') if trimmed.starts_with("/>") => {}
                    Some(_) => out.push(' '),
                    None => {
                        out.push(' ');
                        return (header.len(), out);
                    }
                }
            }
            c => out.push(c),
        }
    }
    (header.len(), out)
}
