# vectory-ffi — the C ABI over the vectory SVG model

The native engine behind the Ruby gem's runtime switch (`Vectory::Runtime`,
`lib/vectory/runtime.rb`): decode + serialize with the two serializer
profiles, and the transform pipeline, over a Fiddle-loadable cdylib.

## Binding decision (lane 3, claricle/vectory#99)

**Fiddle + cdylib, svgpng-style — not an rb-sys gem extension.** The org has
both precedents; this lane takes the lighter one:

* `fiddle` is stdlib: the optional native lane adds **zero** runtime gem
  dependencies, and a plain source install never needs a Rust toolchain. An
  rb-sys extension would put a compile requirement on every install (or
  force the platform-gem matrix to exist before the lane is usable at all).
* The ABI passes structs through caller-allocated pointers and returns plain
  `int32` statuses — the convention Fiddle handles without struct-return
  tricks, proven by svgpng's shipped binding on all seven platform targets
  vectory already depends on.
* The vendored-cdylib skeleton still follows the etassia-rs machinery:
  `rakelib/native.rake` builds with the rb-sys toolchain table
  (`RbSys::ToolchainInfo`) for `CARGO_BUILD_TARGET`-style cross builds,
  vendors the dylib into `lib/vectory/native/`, and packages platform gems.
  rb-sys is build-time tooling there, never a runtime dependency.

## ABI

```c
typedef struct VectoryBuf { uint8_t *ptr; size_t len; } VectoryBuf;

int32_t vectory_v1_roundtrip(const char *input, size_t input_len,
                             int32_t profile, VectoryBuf *out);
int32_t vectory_v1_pipeline(const char *input, size_t input_len,
                            const char *spec, size_t spec_len,
                            VectoryBuf *out);
void    vectory_v1_free(VectoryBuf *out);
```

* status `0` — success; `out` holds the result bytes.
* status nonzero — failure; `out` holds JSON `{"class": ..., "message": ...}`
  where `class` names the Ruby error class the loader raises
  (`ParsingError`, `ArgumentError`, `ConversionError`,
  `ForbiddenExternalReferenceError`).
* The native side owns `out`'s bytes until `vectory_v1_free`; Ruby copies
  them out first and always frees (the svgpng ownership discipline).

`profile`: `0` = `RubyXml` (the vectory writer's bytes), `1` =
`EmfsvgSvgBuilder` (emfsvg's renderer serializer bytes).

`spec` (pipeline): the canonical step spec both lanes interpret —
`{"steps": [{"name": "graft_into", "host_content": ...}, {"name":
"disambiguate", "index": 1, "duplicate_ids": {...}}, {"name":
"suffix_classes", "index": 1}, {"name": "bind_namespaces", "namespaces":
{"bind": {...}, "unbind": [...], "remap": {...}}}]}`.
`lib/vectory/runtime_steps.rb` in the gem keeps the Ruby interpretation of
the same shape.

## Statuses

| code | meaning |
| ---- | ------- |
| 0 | success |
| 1 | invalid argument (bad UTF-8, unknown profile, malformed spec) |
| 2 | parsing (XML rejected; class `ParsingError` / `ForbiddenExternalReferenceError`) |
| 3 | transform (sequence, scope, namespace requests; class `ArgumentError` / `ConversionError`) |
