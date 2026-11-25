//! THE PARITY GATE (claricle/emf#88): for the emfsvg parity corpus,
//! EMF → VIM → SVG-model serialization must equal the Ruby renderer's SVG
//! bytes FILE-BY-FILE.
//!
//! Environment:
//! * `EMF_FIXTURE_ROOT` — the pinned external emf checkout (the corpus
//!   sources live under spec/fixtures/{emf,emf-ea,simple});
//! * `EMF_GOLDEN_ROOT` — the emfsvg checkout's spec/fixtures/golden
//!   directory (the Ruby renderer's reference bytes).
//!
//! When either variable is unset the gate skips (CI without the pinned
//! corpus is not a failure); a local run reports the counts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eikogram_to_svg::drawing_to_svg;
use emf_to_eikogram::{render, RenderOptions};

/// The SVG model's documented non-representable quirk normalization: the
/// reference renderer emits whitespace inside the tag grammar that XML
/// parsing discards (extra spaces between attributes, `style ="..."`,
/// space before `/>`). Byte comparison applies the same normalization to
/// both sides, exactly as the vectory crate's emfsvg fidelity contract
/// defines it. Attribute values are never touched (quoted regions are
/// verbatim), so quirks inside values — `translate(0.0000, 0.00 00)`, path
/// `d` spacing — compare byte-for-byte.
fn normalize_quirks(input: &str) -> String {
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

/// Normalizes one markup header: whitespace outside quoted attribute values
/// collapses to a single space, disappears before `=`, `/>`, and `>`.
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

struct Corpus {
    /// name → EMF bytes
    sources: BTreeMap<String, Vec<u8>>,
    /// name → golden SVG bytes
    goldens: BTreeMap<String, String>,
}

fn load_corpus() -> Option<Corpus> {
    let fixture_root = std::env::var("EMF_FIXTURE_ROOT").ok()?;
    let golden_root = std::env::var("EMF_GOLDEN_ROOT").ok()?;
    let fixture_root = PathBuf::from(fixture_root);
    let golden_root = PathBuf::from(golden_root);

    let mut sources = BTreeMap::new();
    for dir in [
        "spec/fixtures/emf",
        "spec/fixtures/emf-ea",
        "spec/fixtures/simple",
    ] {
        let dir = fixture_root.join(dir);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "emf") {
                let name = path
                    .file_name()?
                    .to_string_lossy()
                    .trim_end_matches(".emf")
                    .to_string();
                let bytes = std::fs::read(&path).ok()?;
                sources.insert(name, bytes);
            }
        }
    }

    let mut goldens = BTreeMap::new();
    let entries = std::fs::read_dir(&golden_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "svg") {
            let name = path
                .file_name()?
                .to_string_lossy()
                .trim_end_matches(".svg")
                .to_string();
            let content = std::fs::read_to_string(&path).ok()?;
            goldens.insert(name, content);
        }
    }
    Some(Corpus { sources, goldens })
}

/// Renders one EMF through the transformer chain; the error string reports
/// the failure class.
fn render_svg(emf_bytes: &[u8]) -> Result<String, String> {
    let document = emf_core::parse(emf_bytes, &emf_core::ParseLimits::default())
        .map_err(|e| format!("parse: {e}"))?;
    let drawing =
        render(&document, &RenderOptions::default()).map_err(|e| format!("render: {e}"))?;
    Ok(drawing_to_svg(&drawing))
}

/// First-difference report for a mismatch (the gate's diagnostics).
fn first_diff(expected: &str, actual: &str) -> String {
    let expected_bytes = expected.as_bytes();
    let actual_bytes = actual.as_bytes();
    let common = expected_bytes
        .iter()
        .zip(actual_bytes.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let context_start = common.saturating_sub(200);
    let context_end = |s: &[u8], at: usize| -> String {
        let end = (at + 200).min(s.len());
        String::from_utf8_lossy(&s[context_start.min(end)..end]).to_string()
    };
    format!(
        "first difference at byte {common}:\n  golden: …{}…\n  rust:   …{}…",
        context_end(expected_bytes, common),
        context_end(actual_bytes, common)
    )
}

#[test]
fn parity_corpus_byte_gate() {
    let Some(corpus) = load_corpus() else {
        eprintln!(
            "parity corpus not configured (EMF_FIXTURE_ROOT / EMF_GOLDEN_ROOT unset); skipping"
        );
        return;
    };

    let mut matched_raw: Vec<String> = Vec::new();
    let mut matched_normalized: Vec<String> = Vec::new();
    let mut mismatches: Vec<(String, String)> = Vec::new();
    let mut unported: Vec<(String, String)> = Vec::new();

    for (name, emf_bytes) in &corpus.sources {
        let Some(golden) = corpus.goldens.get(name) else {
            continue;
        };
        match render_svg(emf_bytes) {
            Ok(actual) => {
                if actual == *golden {
                    matched_raw.push(name.clone());
                    matched_normalized.push(name.clone());
                } else {
                    let golden_normalized = normalize_quirks(golden);
                    let actual_normalized = normalize_quirks(&actual);
                    if golden_normalized == actual_normalized {
                        matched_normalized.push(name.clone());
                    } else {
                        let diff = first_diff(&golden_normalized, &actual_normalized);
                        mismatches.push((name.clone(), diff));
                    }
                }
            }
            Err(error) => unported.push((name.clone(), error)),
        }
    }

    eprintln!("=== EMF → VIM → SVG byte-parity gate ===");
    eprintln!(
        "corpus: {} sources, {} goldens",
        corpus.sources.len(),
        corpus.goldens.len()
    );
    eprintln!("match (raw bytes):       {}", matched_raw.len());
    eprintln!("match (quirk-normalized): {}", matched_normalized.len());
    eprintln!("mismatch: {}", mismatches.len());
    eprintln!("unported: {}", unported.len());
    for (name, error) in unported.iter().take(12) {
        eprintln!("  unported {name}: {error}");
    }
    for (name, diff) in mismatches.iter().take(12) {
        eprintln!("  mismatch {name}: {diff}");
    }

    // THE GATE: zero mismatches, zero unported. Failing files print their
    // first-diff analysis above.
    assert!(
        mismatches.is_empty() && unported.is_empty(),
        "parity gate: match={} mismatch={} unported={}",
        matched_normalized.len(),
        mismatches.len(),
        unported.len()
    );
}

/// Per-file gate for a single name (debugging helper; run with
/// PARITY_ONE=<name>).
#[test]
fn parity_single_file() {
    let Ok(name) = std::env::var("PARITY_ONE") else {
        return;
    };
    let Some(corpus) = load_corpus() else { return };
    let Some(emf_bytes) = corpus.sources.get(&name) else {
        panic!("no EMF source for {name}");
    };
    let golden = corpus.goldens.get(&name).expect("golden");
    match render_svg(emf_bytes) {
        Ok(actual) => {
            // Dump the rendered bytes for shell-side diffing.
            if let Ok(out_path) = std::env::var("PARITY_OUT") {
                std::fs::write(&out_path, &actual).expect("write PARITY_OUT");
                eprintln!("rendered bytes written to {out_path}");
            }
            if actual == *golden {
                println!("{name}: raw byte match");
            } else {
                let golden_normalized = normalize_quirks(golden);
                let actual_normalized = normalize_quirks(&actual);
                if golden_normalized == actual_normalized {
                    println!("{name}: quirk-normalized match");
                } else {
                    panic!(
                        "{} mismatch: {}",
                        name,
                        first_diff(&golden_normalized, &actual_normalized)
                    );
                }
            }
        }
        Err(error) => panic!("{name}: {error}"),
    }
}

fn _unused(_: &Path) {}
