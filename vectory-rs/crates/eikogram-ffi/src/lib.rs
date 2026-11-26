//! # eikogram-ffi — the C ABI over the EMF → VIM → SVG conversion
//!
//! The native render engine behind the Ruby gem's runtime switch
//! (`Emfsvg::Runtime`, claricle/emf#91): EMF metafile bytes in, the emfsvg
//! renderer's SVG bytes out, over a Fiddle-loadable cdylib. The conversion
//! runs the full landed pipeline (the conversion-hub law — no crate
//! re-implements it):
//!
//! ```text
//! EMF bytes -> emf-core (the EMF model) -> emf-to-eikogram (the VIM)
//!           -> eikogram-to-svg (the SVG model, EmfsvgSvgBuilder profile)
//!           -> SVG bytes
//! ```
//!
//! ## Binding decision
//!
//! Fiddle + cdylib with a caller-allocated output buffer (the svgpng /
//! vectory-ffi precedent), NOT an rb-sys gem extension:
//!
//! * `fiddle` is stdlib — the optional native lane adds zero runtime gem
//!   dependencies, and a source install never needs a Rust toolchain;
//! * the ABI passes structs through caller-allocated pointers and returns
//!   plain `int32` statuses — the convention Fiddle handles without
//!   libffi struct-return tricks;
//! * one binding style across the program: the parse lane (emf-ffi,
//!   snapshot v1→v3) and this render lane agree on ownership and status
//!   discipline. The contract is documented as snapshot v4 beside the
//!   earlier snapshots: emf-rs/crates/emf-ffi/snapshot-v4.md in
//!   claricle/emf.
//!
//! ## The ABI
//!
//! ```c
//! typedef struct EikogramBuf { uint8_t *ptr; size_t len; } EikogramBuf;
//! int32_t eikogram_v1_abi_version(void);
//! int32_t eikogram_v1_render_emf(const uint8_t *emf, size_t emf_len,
//!                                EikogramBuf *out);
//! void    eikogram_v1_free(EikogramBuf *out);
//! ```
//!
//! * status 0 — success; `out` holds the SVG bytes.
//! * status nonzero — failure; `out` holds a JSON body
//!   `{"class": <Ruby error class name>, "message": ...}`. The Ruby loader
//!   maps the class name to its own error classes.
//! * the native side owns `out`'s bytes until `eikogram_v1_free`; Ruby
//!   copies them out first and always frees (the svgpng ownership
//!   discipline).
//!
//! ## Statuses
//!
//! | status | meaning    | error-body `class` names                      |
//! |--------|------------|-----------------------------------------------|
//! | 0      | success    | —                                             |
//! | 1      | bad call   | `ArgumentError`                               |
//! | 2      | parse      | `ParseError` (the emf-core parse failure)     |
//! | 3      | render     | `RenderError`, `ResourceLimitError`           |
//! | 4      | serialize  | `RenderError` (the SVG model's error)         |
//!
//! ## Versioning evolution
//!
//! The C ABI is version 1 (`eikogram_v1_abi_version`), checked by the
//! loader before any call. Evolution follows the snapshot v1→v3 pattern:
//! new capability means a NEW suffixed entry point (old ones keep their
//! bytes), never an in-place change — options transport (the Ruby
//! renderer's width/height/namespace) and the reverse SVG→EMF direction
//! (claricle/emf#90) ride that pattern.
//!
//! # Boundaries
//!
//! This crate adds no rendering semantics of its own: it validates
//! pointers, sequences the pipeline, and carries failures across the
//! boundary. Every drawing decision belongs to the transformers.

use std::ffi::c_char;
use std::slice;

use eikogram_to_svg::drawing_to_svg_string;
use emf_to_eikogram::{render, RenderFailure, RenderOptions};

/// Statuses the ABI answers with. Distinct from the JSON error body so the
/// loader can raise named Ruby classes without string-matching.
const STATUS_OK: i32 = 0;
const STATUS_INVALID_ARGUMENT: i32 = 1;
const STATUS_PARSING: i32 = 2;
const STATUS_RENDER: i32 = 3;
const STATUS_SERIALIZE: i32 = 4;

/// The loader checks this before any call.
const ABI_VERSION: i32 = 1;

/// The caller-allocated result buffer (svgpng's ownership shape).
#[repr(C)]
pub struct EikogramBuf {
    ptr: *mut u8,
    len: usize,
}

impl EikogramBuf {
    const EMPTY: EikogramBuf = EikogramBuf {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
}

/// A conversion failure from anywhere in the pipeline.
enum ConversionFailure {
    Parsing(emf_core::ParseFailure),
    Rendering(RenderFailure),
    Serializing(vectory::Error),
}

impl From<emf_core::ParseFailure> for ConversionFailure {
    fn from(failure: emf_core::ParseFailure) -> Self {
        ConversionFailure::Parsing(failure)
    }
}

impl From<RenderFailure> for ConversionFailure {
    fn from(failure: RenderFailure) -> Self {
        ConversionFailure::Rendering(failure)
    }
}

impl From<vectory::Error> for ConversionFailure {
    fn from(error: vectory::Error) -> Self {
        ConversionFailure::Serializing(error)
    }
}

/// The conversion, model-driven end to end: parse, lift, transform,
/// serialize. The only sequence point of the whole crate.
fn render_emf_to_svg(emf: &[u8]) -> Result<String, ConversionFailure> {
    let document = emf_core::parse(emf, &emf_core::ParseLimits::default())?;
    let drawing = render(&document, &RenderOptions::default())?;
    Ok(drawing_to_svg_string(&drawing)?)
}

/// The ABI version the loader verifies before its first call.
#[no_mangle]
pub extern "C" fn eikogram_v1_abi_version() -> i32 {
    ABI_VERSION
}

/// Render EMF metafile bytes to the emfsvg renderer's SVG bytes.
///
/// # Safety
///
/// `emf` must point to `emf_len` readable bytes (a null pointer with a
/// zero length reads as the empty input, which fails downstream parsing
/// loudly); `out` must point to a writable `EikogramBuf`. The result lives
/// until `eikogram_v1_free`.
#[no_mangle]
pub unsafe extern "C" fn eikogram_v1_render_emf(
    emf: *const c_char,
    emf_len: usize,
    out: *mut EikogramBuf,
) -> i32 {
    let Some(emf) = read_input(emf, emf_len) else {
        return write_invalid_argument(out, "emf input is not readable");
    };
    respond(render_emf_to_svg(emf), out)
}

/// Release a buffer this library wrote into `out`. Safe to call with the
/// zeroed buffer Ruby starts from (null pointer, zero length).
///
/// # Safety
///
/// `out` must have been written by this library or zeroed; no concurrent use.
#[no_mangle]
pub unsafe extern "C" fn eikogram_v1_free(out: *mut EikogramBuf) {
    if out.is_null() {
        return;
    }
    let buf = std::ptr::read(out);
    if buf.ptr.is_null() || buf.len == 0 {
        return;
    }
    drop(Vec::from_raw_parts(buf.ptr, buf.len, buf.len));
    std::ptr::write(out, EikogramBuf::EMPTY);
}

/// Views the caller's bytes. EMF is binary: no UTF-8 gate here (the
/// vectory-ffi XML inputs have one — this lane must not).
///
/// # Safety
///
/// `input` must point to `len` readable bytes or be null with `len == 0`.
unsafe fn read_input<'a>(input: *const c_char, len: usize) -> Option<&'a [u8]> {
    if input.is_null() {
        return if len == 0 { Some(&[]) } else { None };
    }
    Some(slice::from_raw_parts(input.cast::<u8>(), len))
}

/// Writes the result or the JSON error body; never panics across the ABI.
fn respond(result: Result<String, ConversionFailure>, out: *mut EikogramBuf) -> i32 {
    match result {
        Ok(body) => write_bytes(out, body.into_bytes(), STATUS_OK),
        Err(failure) => {
            let (status, body) = error_body(&failure);
            write_bytes(out, body.to_string().into_bytes(), status)
        }
    }
}

/// The status and the JSON error body for one failure. The resource-limit
/// body carries structured fields so the Ruby loader can raise the named
/// error with its keyword arguments (the message alone is not enough).
fn error_body(failure: &ConversionFailure) -> (i32, serde_json::Value) {
    match failure {
        ConversionFailure::Parsing(failure) => (
            STATUS_PARSING,
            serde_json::json!({ "class": "ParseError", "message": failure.to_string() }),
        ),
        ConversionFailure::Rendering(RenderFailure::Dimension(message)) => (
            STATUS_RENDER,
            serde_json::json!({ "class": "RenderError", "message": message }),
        ),
        ConversionFailure::Rendering(RenderFailure::ResourceLimit { limit }) => (
            STATUS_RENDER,
            serde_json::json!({
                "class": "ResourceLimitError",
                "message": format!("record_count limit exceeded (over {limit})"),
                "resource": "record_count",
                "limit": limit,
                "actual": null,
            }),
        ),
        ConversionFailure::Serializing(error) => (
            STATUS_SERIALIZE,
            serde_json::json!({ "class": "RenderError", "message": error.to_string() }),
        ),
    }
}

/// Maps a pipeline failure to its status and the Ruby error class the
/// loader raises (the emfsvg vocabulary; unknown names fall back to the
/// loader's native error, never a silent pass).
fn write_invalid_argument(out: *mut EikogramBuf, message: &str) -> i32 {
    let body = serde_json::json!({ "class": "ArgumentError", "message": message });
    write_bytes(out, body.to_string().into_bytes(), STATUS_INVALID_ARGUMENT)
}

/// Hands the bytes to the caller and records them in `out`. An empty body
/// records the zeroed buffer, which `eikogram_v1_free` releases harmlessly.
fn write_bytes(out: *mut EikogramBuf, bytes: Vec<u8>, status: i32) -> i32 {
    if out.is_null() {
        return status;
    }
    let written = if bytes.is_empty() {
        EikogramBuf::EMPTY
    } else {
        let mut bytes = bytes.into_boxed_slice();
        let ptr = bytes.as_mut_ptr();
        let len = bytes.len();
        std::mem::forget(bytes);
        EikogramBuf { ptr, len }
    };
    unsafe {
        std::ptr::write(out, written);
    }
    status
}
