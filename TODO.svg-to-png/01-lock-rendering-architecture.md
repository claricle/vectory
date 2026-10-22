# 01 — Codify the complete rendering contract

## Work

In the new `claricle/svgpng` repository, scaffold the `Svgpng` namespace and record the fixed architecture before native packaging begins. Add `docs/decisions/svg-to-png.md`, a compact support section in `README.adoc`, and public result/error/input shapes under `lib/svgpng/`. All paths in this card are relative to the `svgpng` repository.

Define `Svgpng.render(svg_bytes, profile:, fonts:, fallback_fonts:)` as the byte-oriented boundary. `fonts:` is a hash from requested CSS family to one or more `Svgpng::FontFace` values; `fallback_fonts:` is an ordered array of faces; each face contains immutable bytes and a diagnostic `source_id`, while family/style/weight/stretch and glyph coverage are derived from its OpenType metadata. The result exposes PNG bytes, dimensions, and structured warnings without exposing native pointers or resvg types. There is no external-resource input API.

Record the owned native pipeline and proposed paths: `native/svgpng_shim/` defines the sole Fiddle ABI; a Claricle resvg fork based on v0.48.1 exposes the narrow font-resolution hook and rejects non-inline external references during parsing; the shim statically links that fork, libpng 1.6.59, and zlib 1.3.2. `vendor/manifest.yml` pins source URLs, commits/versions, checksums, licences, toolchain, build flags, and both patch digests. These paths are proposed until the new repository exists and must be adjusted in the decision document if its scaffold differs.

Fix packaging as seven same-version native gems plus one `ruby`-platform shell gem. The shell exposes the same Ruby API and errors without loading a shared library; it permits installation on other platforms and raises `Svgpng::UnsupportedPlatformError` only when rendering. Document native-over-shell resolution on supported targets and clean unsupported-platform installation as part of the public compatibility contract.

Fix the PNG byte profile. resvg supplies premultiplied RGBA8 in top-to-bottom row order. For each channel, the shim emits zero when alpha is zero and otherwise computes `min(255, (premultiplied * 255 + alpha / 2) / alpha)` using checked integer arithmetic. Treat the resulting samples as straight-alpha sRGB values; perform no gamma or profile transform. Encode colour type 6, 8-bit depth, no interlace, filter `PNG_FILTER_NONE`, zlib level 9 with default strategy/window 15/memory level 8, and a 32 KiB libpng compression buffer. Emit only the PNG signature, `IHDR`, deterministic one-or-more `IDAT` chunks, and `IEND`; omit time, text, DPI, gamma, chromaticity, sRGB, ICC, EXIF, and every other ancillary chunk. Alpha/background compositing occurs in resvg before this conversion.

Define one complete render-profile algorithm in `lib/svgpng/render_profile.rb` and mirror it in the decision document:

1. Accept a hash with only `:width`, `:height`, `:scale`, and `:background`. `width` and `height` are positive Ruby integers; `scale` is a positive finite base-10 decimal string parsed to an exact rational; `background` is a string in the formats below. Reject string/unknown keys, booleans, zero, negative, NaN/infinite, fractional pixel dimensions, values beyond the documented dimension/pixel limits, and any profile combining `scale` with `width` or `height`.
2. Resolve one positive intrinsic source viewport in the pinned parser, with CSS px as one source unit, physical CSS lengths converted at 96 dpi, and percentage root width/height resolved against a 300×150 default viewport. Reject other relative root sizing such as `em`, `rem`, `ex`, or `calc()` rather than consulting font or host state. Two positive resolved root dimensions win. With one resolved dimension and a positive `viewBox`, derive the missing axis from the `viewBox` aspect ratio; without a usable `viewBox`, the missing width or height is 300 or 150 respectively. With neither root dimension, use a positive `viewBox`'s width and height; without one, use 300×150. Reject zero/negative/non-finite resolved dimensions. resvg still applies the document's own `preserveAspectRatio` (default `xMidYMid meet`) when painting the `viewBox` inside this intrinsic viewport.
3. Determine output size in this precedence order: both `width` and `height` produce that exact canvas; width alone derives height from the intrinsic ratio; height alone derives width; `scale` alone multiplies both intrinsic axes; an empty profile uses both intrinsic axes. Round every derived positive dimension with `floor(value + 1/2)` and clamp only a positive value below one pixel to one. Check dimensions, pixel count, stride, and buffer arithmetic after rounding and before allocation.
4. Preserve the intrinsic viewport's aspect ratio. For an exact two-dimensional canvas, use `min(output_width / intrinsic_width, output_height / intrinsic_height)` and center the resulting viewport with translations `(output_width - scaled_width) / 2` and `(output_height - scaled_height) / 2`; the pinned native transform receives those values. Never stretch axes independently. This outer fit is separate from the SVG's own `preserveAspectRatio` behavior.
5. Default `background` to transparent black. Accept only `transparent`, `#RRGGBB`, or `#RRGGBBAA`; expand the six-digit form with alpha `FF`. Convert each background colour channel to premultiplied form with `(channel * alpha + 127) / 255` using integer division, fill the entire canvas first (including any letterbox area), then have resvg composite the SVG source-over into it. Perform no second background operation after Card 01's unpremultiplication step.

Pin observable profile fixtures for: no dimensions or `viewBox` (300×150); `viewBox="0 0 20 10"` without dimensions (20×10); width 200 with that `viewBox` and no height (200×100); width-only, height-only, scale-only, and exact two-axis requests; an aspect-ratio mismatch with centered background padding; half-pixel derived dimensions; transparent and explicit-alpha backgrounds; and every rejected combination/value. Fix warning order, external-reference behavior, and enforceable limits alongside this profile. Raw XML `DOCTYPE` declarations are rejected before DOM construction or entity expansion. Same-document fragments and inline `data:` values are allowed; relative and absolute paths, network and protocol-relative URLs, `file:` URLs, CSS imports, and every other non-inline reference are rejected before rendering. Bounds cover SVG bytes, font count and bytes, requested dimensions, pixel count, row stride, and output-buffer arithmetic before the associated allocation. There is no in-process render-time limit because the synchronous resvg call has no safe cancellation API; process isolation for hostile-input deadlines is documented as caller responsibility.

Define stable errors only for behavior the boundary can observe: malformed/invalid input, font metadata/duplicate/missing-family/missing-glyph failures, forbidden external references, profile and pre-allocation limits, encoding/native failures, and unsupported platforms. Static constructs unsupported by the pinned resvg version retain resvg's documented behavior; the gem does not promise a blanket error for every ignored construct.

## Done when

- `docs/decisions/svg-to-png.md` records the patched resvg v0.48.1 base, owned shim, libpng 1.6.59, zlib 1.3.2, Fiddle, proposed paths, all seven native targets, and the same-version `ruby` shell package.
- The public `Svgpng` boundary defines the exact SVG/profile/font inputs and PNG/dimension/warning outputs without exposing engine-specific types or an external-resource injection API.
- The premultiplied-to-straight RGBA formula and every PNG bit-depth, colour, filtering, compression, chunk, metadata, and colour-treatment choice are fixed.
- The profile contract fixes intrinsic-size defaults and resolution, validation, width/height/scale precedence, half-up rounding, aspect-ratio-preserving fit and centering, and premultiplied-canvas background composition.
- Focused cases prove 300×150 fallback, `viewBox`-derived size, one-axis derivation, exact two-axis fit/padding, scale, half-pixel rounding, transparent/explicit backgrounds, and invalid profile rejection.
- Font-resolution enforcement/diagnostics and parser-side external-reference rejection have named owning components and explicit native/Ruby responsibilities.
- Only enforceable preflight/pre-allocation limits are promised, while wall-clock cancellation and unsupported-static-feature behavior are documented accurately.
- Error categories and unsupported-platform behavior are explicit and testable, including install/require success through the shell before render raises.

```sh
bundle exec rspec spec/render_profile_spec.rb && test -f docs/decisions/svg-to-png.md && rg -n "v0\.48\.1|libpng 1\.6\.59|zlib 1\.3\.2|premultiplied|PNG_FILTER_NONE|32 KiB|300×150|half-up|aspect ratio|background|font-resolution|external reference|wall-clock|x64-mingw-ucrt" docs/decisions/svg-to-png.md README.adoc
```

## Status
State: todo
PR: none

## Effort

2–3 engineer-days.
