# eikogram-ffi — the C ABI over the EMF → VIM → SVG conversion

The native render engine behind the Ruby gem's runtime switch
(`Emfsvg::Runtime`, claricle/emf#91): EMF metafile bytes in, the emfsvg
renderer's SVG bytes out, over a Fiddle-loadable cdylib. The conversion is
the full landed pipeline, composed here — never re-implemented:

```text
EMF bytes -> emf-core (the EMF model, claricle/emf)
          -> emf-to-eikogram (the VIM, the conversion hub)
          -> eikogram-to-svg (the SVG model, EmfsvgSvgBuilder profile)
          -> SVG bytes
```

## Binding decision

**Fiddle + cdylib, svgpng-style — the vectory-ffi convention, not an rb-sys
gem extension.** One binding style across the program: the parse lane
(emf-ffi, snapshot v1→v3 in claricle/emf) and this render lane agree on
plain-`int32` statuses, caller-allocated output buffers, and the
copy-then-free ownership discipline. The contract is documented as snapshot
v4 beside the earlier snapshots: `emf-rs/crates/emf-ffi/snapshot-v4.md` in
claricle/emf.

## ABI

```c
typedef struct EikogramBuf { uint8_t *ptr; size_t len; } EikogramBuf;

int32_t eikogram_v1_abi_version(void);
int32_t eikogram_v1_render_emf(const uint8_t *emf, size_t emf_len,
                               EikogramBuf *out);
void    eikogram_v1_free(EikogramBuf *out);
```

* status `0` — success; `out` holds the SVG bytes.
* status nonzero — failure; `out` holds JSON `{"class": ..., "message": ...}`
  where `class` names the Ruby error class the loader raises:
  `ParseError` (the emf-core parse failure), `RenderError` /
  `ResourceLimitError` (the transformer's raised conditions), `RenderError`
  for the SVG model's serialization failures.
* The native side owns `out`'s bytes until `eikogram_v1_free`; Ruby copies
  them out first and always frees (the svgpng ownership discipline).
* A null `emf` with `emf_len == 0` reads as the empty input and answers the
  named parse failure — EMF is binary, so unlike vectory-ffi there is no
  UTF-8 gate on the input.

## Versioning evolution

The C ABI is version 1, checked by the loader before its first call.
Evolution follows the snapshot v1→v3 pattern: new capability means a NEW
suffixed entry point (old ones keep their bytes), never an in-place change.
Planned on that pattern: options transport (the renderer's
width/height/namespace) and the reverse SVG→EMF direction (claricle/emf#90).
