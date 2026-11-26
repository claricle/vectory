//! The FFI boundary is byte-transparent (claricle/emf#91): rendering through
//! the LOADED cdylib must equal the direct transformer chain
//! (emf-core -> emf-to-eikogram -> eikogram-to-svg) byte for byte, over a
//! deterministic corpus subset. An empty/short input answers the named
//! parse failure; the free path accepts the zeroed buffer.
//!
//! The cdylib is resolved in this order: `EIKOGRAM_FFI_TEST_LIBRARY`
//! (exact path), then the workspace's `target/release`, then
//! `target/debug`. `cargo test` does not build cdylib artifacts, so the
//! boundary examples skip loudly when no library exists — build with
//! `cargo build -p eikogram-ffi` (CI's native-runtime job and the local
//! flow do) to exercise them.
//! The corpus test additionally skips unless EMF_FIXTURE_ROOT names the
//! pinned external emf checkout (CI without the pinned corpus is not a
//! failure).

use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

const STATUS_OK: i32 = 0;
const STATUS_PARSING: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct Buf {
    ptr: *mut u8,
    len: usize,
}

impl Buf {
    const ZEROED: Buf = Buf {
        ptr: std::ptr::null_mut(),
        len: 0,
    };

    fn body(&self) -> Vec<u8> {
        if self.ptr.is_null() || self.len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }.to_vec()
        }
    }
}

struct Binding {
    library: Library,
}

impl Binding {
    /// Opens the built cdylib, or `None` when none exists.
    fn open() -> Option<Self> {
        let path = library_path()?;
        // SAFETY: the cdylib cargo built for this workspace; no global
        // constructor requirements.
        let library = unsafe { Library::new(&path) }
            .unwrap_or_else(|error| panic!("cannot load {}: {error}", path.display()));
        Some(Binding { library })
    }

    fn abi_version(&self) -> i32 {
        // SAFETY: the ABI's plain-int getter.
        unsafe {
            let func: Symbol<unsafe extern "C" fn() -> i32> =
                self.library.get(b"eikogram_v1_abi_version").unwrap();
            func()
        }
    }

    /// One render call owning its buffers until the copy completes.
    fn render_emf(&self, emf: &[u8]) -> (i32, Vec<u8>) {
        let mut out = Buf::ZEROED;
        // SAFETY: the ABI's documented contract — readable input, writable
        // out, result released below.
        let status = unsafe {
            let func: Symbol<unsafe extern "C" fn(*const u8, usize, *mut Buf) -> i32> =
                self.library.get(b"eikogram_v1_render_emf").unwrap();
            let input = if emf.is_empty() {
                std::ptr::null()
            } else {
                emf.as_ptr()
            };
            func(input, emf.len(), &mut out)
        };
        let body = out.body();
        unsafe {
            let func: Symbol<unsafe extern "C" fn(*mut Buf)> =
                self.library.get(b"eikogram_v1_free").unwrap();
            func(&mut out);
        }
        (status, body)
    }
}

fn library_path() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os("EIKOGRAM_FFI_TEST_LIBRARY") {
        let path = PathBuf::from(override_path);
        return path.is_file().then_some(path);
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    ["release", "debug"].into_iter().find_map(|profile| {
        let base = target.join(profile);
        [
            "libeikogram_ffi.dylib",
            "libeikogram_ffi.so",
            "eikogram_ffi.dll",
        ]
        .into_iter()
        .map(|name| base.join(name))
        .find(|path| path.is_file())
    })
}

/// The direct transformer chain (no FFI) — the reference the boundary is
/// checked against.
fn direct_render(emf: &[u8]) -> Result<String, String> {
    let document = emf_core::parse(emf, &emf_core::ParseLimits::default())
        .map_err(|error| format!("parse: {error}"))?;
    let drawing = emf_to_eikogram::render(&document, &emf_to_eikogram::RenderOptions::default())
        .map_err(|error| format!("render: {error}"))?;
    Ok(eikogram_to_svg::drawing_to_svg(&drawing))
}

/// The built cdylib, or `None` when none exists (the examples skip loudly;
/// CI's plain `cargo test` does not build cdylib artifacts, while the
/// native-runtime job and the local `cargo build -p eikogram-ffi` flow do).
fn built_library() -> Option<Binding> {
    Binding::open()
}

#[test]
fn abi_version_is_one() {
    let Some(binding) = built_library() else {
        eprintln!(
            "skipping: no eikogram-ffi cdylib found; run \
             `cargo build -p eikogram-ffi` first (or set \
             EIKOGRAM_FFI_TEST_LIBRARY)"
        );
        return;
    };
    assert_eq!(binding.abi_version(), 1);
}

#[test]
fn free_accepts_the_zeroed_buffer() {
    let Some(binding) = built_library() else {
        eprintln!(
            "skipping: no eikogram-ffi cdylib found; run \
             `cargo build -p eikogram-ffi` first (or set \
             EIKOGRAM_FFI_TEST_LIBRARY)"
        );
        return;
    };
    let mut out = Buf::ZEROED;
    // SAFETY: the documented safe-input case (never written).
    unsafe {
        let func: Symbol<unsafe extern "C" fn(*mut Buf)> =
            binding.library.get(b"eikogram_v1_free").unwrap();
        func(&mut out);
    }
    assert_eq!(out.ptr, std::ptr::null_mut());
}

#[test]
fn an_empty_input_answers_the_named_parse_failure() {
    let Some(binding) = built_library() else {
        eprintln!(
            "skipping: no eikogram-ffi cdylib found; run \
             `cargo build -p eikogram-ffi` first (or set \
             EIKOGRAM_FFI_TEST_LIBRARY)"
        );
        return;
    };
    let (status, body) = binding.render_emf(&[]);
    assert_eq!(status, STATUS_PARSING);
    let parsed: serde_json::Value =
        serde_json::from_slice(&body).expect("the error body is the JSON contract");
    assert_eq!(parsed["class"], "ParseError");
    assert!(!parsed["message"].as_str().unwrap_or_default().is_empty());
}

#[test]
fn the_boundary_is_byte_transparent_over_the_corpus_subset() {
    let Some(fixture_root) = std::env::var_os("EMF_FIXTURE_ROOT") else {
        eprintln!("corpus not configured (EMF_FIXTURE_ROOT unset); skipping");
        return;
    };
    let Some(binding) = built_library() else {
        eprintln!(
            "skipping: no eikogram-ffi cdylib found; run \
             `cargo build -p eikogram-ffi` first (or set \
             EIKOGRAM_FFI_TEST_LIBRARY)"
        );
        return;
    };
    let fixture_root = PathBuf::from(fixture_root);

    let mut subset = Vec::new();
    for (directory, take) in [("emf", 5), ("emf-ea", 5), ("simple", 4)] {
        let dir = fixture_root.join("spec/fixtures").join(directory);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "emf"))
            .collect();
        names.sort();
        subset.extend(names.into_iter().take(take));
    }
    assert!(subset.len() >= 10, "corpus subset expected 10+ files");
    assert!(
        subset.iter().any(|path| {
            path.parent()
                .is_some_and(|parent| parent.ends_with("emf-ea"))
        }),
        "the subset must include an EMF+ (emf-ea) file"
    );

    for path in &subset {
        let emf = std::fs::read(path).expect("corpus file reads");
        let direct = direct_render(&emf).expect("the corpus renders through the direct chain");
        let (status, body) = binding.render_emf(&emf);
        assert_eq!(status, STATUS_OK, "{} answers success", path.display());
        assert_eq!(
            String::from_utf8_lossy(&body),
            direct,
            "the FFI path is byte-identical to the direct chain for {}",
            path.display()
        );
    }
}
