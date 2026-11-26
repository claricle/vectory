//! # vectory-ffi — the C ABI over the vectory SVG model
//!
//! Lane 3 (claricle/vectory#99): the native engine behind the Ruby gem's
//! runtime switch (`Vectory::Runtime`, lib/vectory/runtime.rb).
//!
//! ## Binding decision
//!
//! Fiddle + cdylib with a caller-allocated output buffer (the svgpng
//! precedent, proven in-org on all seven shipping targets), NOT an rb-sys
//! gem extension:
//!
//! * `fiddle` is stdlib — the optional native lane adds zero runtime gem
//!   dependencies, and a source install never needs a Rust toolchain;
//! * the ABI passes structs through caller-allocated pointers and returns
//!   plain `int32` statuses, the convention Fiddle handles without libffi
//!   struct-return tricks;
//! * the vendored-cdylib skeleton (rakelib/native.rake, rb-sys toolchain
//!   table, `lib/vectory/native/`) still backs the platform-gem builds, so
//!   the cross-gem matrix follow-up inherits the etassia-rs machinery with
//!   rb-sys as build-time tooling only.
//!
//! ## The ABI
//!
//! ```c
//! typedef struct VectoryBuf { uint8_t *ptr; size_t len; } VectoryBuf;
//! int32_t vectory_v1_roundtrip(const char *input, size_t input_len,
//!                              int32_t profile, VectoryBuf *out);
//! int32_t vectory_v1_pipeline(const char *input, size_t input_len,
//!                             const char *spec, size_t spec_len,
//!                             VectoryBuf *out);
//! void vectory_v1_free(VectoryBuf *out);
//! ```
//!
//! * status 0 — success; `out` holds the result bytes.
//! * status nonzero — failure; `out` holds a JSON body
//!   `{"class": <Ruby error class name>, "message": ...}`. The Ruby loader
//!   maps the class name to its own error classes.
//! * the native side owns `out`'s bytes until `vectory_v1_free`; Ruby copies
//!   them out first and always frees (the svgpng ownership discipline).
//!
//! `profile`: 0 = `Profile::RubyXml` (the vectory writer's bytes), 1 =
//! `Profile::EmfsvgSvgBuilder` (emfsvg's renderer serializer bytes).
//!
//! `spec` (pipeline): a JSON document mirroring the canonical step spec the
//! Ruby lane interprets (lib/vectory/runtime_steps.rb keeps the two in
//! step):
//!
//! ```json
//! {"steps": [
//!   {"name": "graft_into", "host_content": "..."},
//!   {"name": "disambiguate", "index": 1, "duplicate_ids": {"r": 2}},
//!   {"name": "suffix_classes", "index": 1},
//!   {"name": "bind_namespaces",
//!    "namespaces": {"bind": {"link": "http://www.w3.org/1999/xlink"},
//!                   "unbind": ["xlink"],
//!                   "remap": {"http://www.w3.org/1999/xlink": "link"}}}
//! ]}
//! ```

use std::collections::HashMap;
use std::ffi::c_char;
use std::slice;

use serde::Deserialize;
use vectory::{Error, SvgTransform, SvgTransformPipeline};

/// Statuses the ABI answers with. Distinct from the JSON error body so the
/// loader can raise named Ruby classes without string-matching.
const STATUS_OK: i32 = 0;
const STATUS_INVALID_ARGUMENT: i32 = 1;
const STATUS_PARSING: i32 = 2;
const STATUS_TRANSFORM: i32 = 3;

/// The caller-allocated result buffer (svgpng's ownership shape).
#[repr(C)]
pub struct VectoryBuf {
    ptr: *mut u8,
    len: usize,
}

impl VectoryBuf {
    const EMPTY: VectoryBuf = VectoryBuf {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
}

/// `Profile::RubyXml` on the wire.
const PROFILE_RUBY_XML: i32 = 0;
/// `Profile::EmfsvgSvgBuilder` on the wire.
const PROFILE_EMFSVG_SVG_BUILDER: i32 = 1;

/// Decode + serialize in one step: XML bytes -> model -> bytes.
///
/// # Safety
///
/// `input` must point to `input_len` readable bytes; `out` must point to a
/// writable `VectoryBuf`. The result lives until `vectory_v1_free`.
#[no_mangle]
pub unsafe extern "C" fn vectory_v1_roundtrip(
    input: *const c_char,
    input_len: usize,
    profile: i32,
    out: *mut VectoryBuf,
) -> i32 {
    let Some(content) = read_input(input, input_len) else {
        return write_invalid_argument(out, "input is not valid UTF-8");
    };
    let Some(profile) = wire_profile(profile) else {
        return write_invalid_argument(out, &format!("unknown profile {profile}"));
    };
    respond(vectory::roundtrip(content, profile), out)
}

/// Execute a transform pipeline: bytes -> the spec'd profile steps -> bytes.
///
/// # Safety
///
/// Same contract as [`vectory_v1_roundtrip`], with `spec`/`spec_len` as the
/// second readable buffer.
#[no_mangle]
pub unsafe extern "C" fn vectory_v1_pipeline(
    input: *const c_char,
    input_len: usize,
    spec: *const c_char,
    spec_len: usize,
    out: *mut VectoryBuf,
) -> i32 {
    let Some(content) = read_input(input, input_len) else {
        return write_invalid_argument(out, "input is not valid UTF-8");
    };
    let Some(spec) = read_input(spec, spec_len) else {
        return write_invalid_argument(out, "spec is not valid UTF-8");
    };
    let steps = match build_steps(spec) {
        Ok(steps) => steps,
        Err(message) => return write_invalid_argument(out, &message),
    };
    let pipeline = match SvgTransformPipeline::new(steps) {
        Ok(pipeline) => pipeline,
        Err(error) => return respond(Err(error), out),
    };
    respond(pipeline.call(content), out)
}

/// Release a buffer this library wrote into `out`. Safe to call with the
/// zeroed buffer Ruby starts from (null pointer, zero length).
///
/// # Safety
///
/// `out` must have been written by this library or zeroed; no concurrent use.
#[no_mangle]
pub unsafe extern "C" fn vectory_v1_free(out: *mut VectoryBuf) {
    if out.is_null() {
        return;
    }
    let buf = std::ptr::read(out);
    if buf.ptr.is_null() || buf.len == 0 {
        return;
    }
    drop(Vec::from_raw_parts(buf.ptr, buf.len, buf.len));
    std::ptr::write(out, VectoryBuf::EMPTY);
}

/// Validates UTF-8 and views the caller's bytes. A null input with zero
/// length reads as the empty string (and fails downstream parsing loudly).
unsafe fn read_input<'a>(input: *const c_char, len: usize) -> Option<&'a str> {
    if input.is_null() {
        return if len == 0 { Some("") } else { None };
    }
    let bytes = slice::from_raw_parts(input.cast::<u8>(), len);
    std::str::from_utf8(bytes).ok()
}

fn wire_profile(profile: i32) -> Option<vectory::serialize::Profile> {
    match profile {
        PROFILE_RUBY_XML => Some(vectory::serialize::Profile::RubyXml),
        PROFILE_EMFSVG_SVG_BUILDER => Some(vectory::serialize::Profile::EmfsvgSvgBuilder),
        _ => None,
    }
}

/// The canonical step spec, JSON on the wire — the same shape the Ruby lane
/// interprets (lib/vectory/runtime_steps.rb keeps the two in step).
#[derive(Debug, Deserialize)]
struct StepSpec {
    name: String,
    #[serde(default)]
    host_content: Option<String>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    duplicate_ids: Option<HashMap<String, usize>>,
    #[serde(default)]
    namespaces: Option<NamespacesSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NamespacesSpec {
    #[serde(default)]
    bind: HashMap<String, String>,
    #[serde(default)]
    unbind: Vec<String>,
    #[serde(default)]
    remap: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct PipelineSpec {
    steps: Vec<StepSpec>,
}

fn build_steps(spec: &str) -> Result<Vec<SvgTransform>, String> {
    let parsed: PipelineSpec =
        serde_json::from_str(spec).map_err(|error| format!("invalid pipeline spec: {error}"))?;
    if parsed.steps.is_empty() {
        return Err("invalid pipeline spec: no steps".to_string());
    }
    parsed
        .steps
        .into_iter()
        .map(|step| build_step(&step))
        .collect()
}

fn build_step(step: &StepSpec) -> Result<SvgTransform, String> {
    match step.name.as_str() {
        "graft_into" => {
            let host_content =
                required(step.host_content.as_deref(), "graft_into", "host_content")?;
            SvgTransform::graft_into(host_content).map_err(|error| error.to_string())
        }
        "disambiguate" => {
            let index = required(step.index, "disambiguate", "index")?;
            let duplicate_ids = step.duplicate_ids.clone().unwrap_or_default();
            Ok(SvgTransform::disambiguate(index, duplicate_ids))
        }
        "suffix_classes" => {
            let index = required(step.index, "suffix_classes", "index")?;
            Ok(SvgTransform::suffix_classes(index))
        }
        "bind_namespaces" => {
            let namespaces = step.namespaces.clone().unwrap_or_default();
            fn borrowed(map: &HashMap<String, String>) -> Vec<(&str, &str)> {
                map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
            }
            let bind = borrowed(&namespaces.bind);
            let unbind = namespaces
                .unbind
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let remap = borrowed(&namespaces.remap);
            SvgTransform::bind_namespaces(&bind, &unbind, &remap).map_err(|error| error.to_string())
        }
        other => Err(format!("unknown step name {other:?}")),
    }
}

fn required<T>(value: Option<T>, step: &str, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("step {step} requires {field}"))
}

/// Writes the result or the JSON error body; never panics across the ABI.
fn respond(result: Result<String, Error>, out: *mut VectoryBuf) -> i32 {
    match result {
        Ok(body) => write_bytes(out, body.into_bytes(), STATUS_OK),
        Err(error) => {
            let (status, class) = classify(&error);
            let body = serde_json::json!({ "class": class, "message": error.to_string() });
            write_bytes(out, body.to_string().into_bytes(), status)
        }
    }
}

fn classify(error: &Error) -> (i32, &'static str) {
    match error {
        Error::Transform(_) => (STATUS_TRANSFORM, "ArgumentError"),
        Error::Conversion(_) => (STATUS_TRANSFORM, "ConversionError"),
        Error::Xml(_) | Error::NoRoot | Error::Parsing(_) => (STATUS_PARSING, "ParsingError"),
        Error::ForbiddenExternalReference(_) => (STATUS_PARSING, "ForbiddenExternalReferenceError"),
    }
}

fn write_invalid_argument(out: *mut VectoryBuf, message: &str) -> i32 {
    let body = serde_json::json!({ "class": "ArgumentError", "message": message });
    write_bytes(out, body.to_string().into_bytes(), STATUS_INVALID_ARGUMENT)
}

/// Hands the bytes to the caller and records them in `out`. An empty body
/// records the zeroed buffer, which `vectory_v1_free` releases harmlessly.
fn write_bytes(out: *mut VectoryBuf, bytes: Vec<u8>, status: i32) -> i32 {
    if out.is_null() {
        return status;
    }
    let written = if bytes.is_empty() {
        VectoryBuf::EMPTY
    } else {
        let mut bytes = bytes.into_boxed_slice();
        let ptr = bytes.as_mut_ptr();
        let len = bytes.len();
        std::mem::forget(bytes);
        VectoryBuf { ptr, len }
    };
    unsafe {
        std::ptr::write(out, written);
    }
    status
}
