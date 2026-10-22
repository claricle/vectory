# SVG-to-PNG raster projection

## Problem

Vectory can parse and serialize SVG and project it to EMF, EPS, and PostScript, but it has no PNG projection. [claricle/vectory#76](https://github.com/claricle/vectory/issues/76) requires a deterministic SVG-to-PNG path whose bytes are stable across supported machines, whose fonts are controlled explicitly, whose native renderer is shipped without compiling during gem installation, and whose result can be consumed directly by `claricle/png_conform`.

resvg v0.48.1 alone does not satisfy that contract: its C API writes premultiplied RGBA pixels rather than PNG bytes, and it does not expose the family/fallback decisions needed for deterministic font diagnostics. The owned native boundary therefore has to cover pixel conversion, PNG encoding, and a narrow font-resolution hook as well as the stock rendering calls.

## Scope

- Build a new sibling `claricle/svgpng` gem, under the `Svgpng` namespace, around a narrowly patched resvg v0.48.1 and a project-owned C ABI shim.
- Statically link resvg, libpng 1.6.59, and zlib 1.3.2 into one Fiddle-loaded shared library per supported platform; ship seven native gems plus a same-version `ruby`-platform shell gem that keeps unsupported platforms installable but raises only when PNG rendering is requested.
- Parse and enforce explicit font inputs, reject every non-inline external reference, and render without consulting system fonts, the current directory, the filesystem, the network, or ambient native libraries.
- Expose SVG-to-PNG as an output projection in Vectory through the public `Svgpng` boundary, including complete CLI inputs and error mapping.
- Prove repeatability with a deliberately small, licensed corpus and byte-for-byte golden PNGs on every initially supported platform.
- Prove that a produced PNG can be handed to a pinned `claricle/png_conform` integration without a format adapter.
- Document release, provenance, failure, font, external-reference, and compatibility behavior for operators and downstream callers.

## Exclusions

- PNG input, decoding, mutation, optimization, or general raster processing in Vectory.
- Dynamic SVG behavior such as scripting, animation, events, or network-loaded resources; unsupported static SVG follows the pinned resvg behavior and is not promised a synthesized per-feature error.
- A general browser-compatible SVG implementation.
- An in-process wall-clock timeout or cancellation guarantee. Callers handling hostile inputs must enforce deadlines by isolating the render in a separate process.
- Changes to the phase-two SVG transform layer in claricle/vectory#77.
- Runtime dependence on Inkscape, ImageMagick, system librsvg, system fonts, or another ambient executable/library.
- Bundled fonts or a built-in fallback font in the initial release.
- Compiling resvg, Rust, a Ruby native extension, libpng, or zlib during gem installation.

## Intended end state

`Svgpng.render` accepts SVG bytes, an explicit render profile, `fonts:`, and ordered `fallback_fonts:`. It returns a result containing deterministic PNG bytes, dimensions, and structured font-substitution warnings without exposing resvg types. Missing fonts or glyphs, non-inline external references, pre-allocation limit failures, native failures, and unsupported platforms raise stable, actionable errors. System-font discovery is disabled, and the renderer has no filesystem or network loading path.

On one of the seven supported targets, RubyGems selects the matching native gem and its single packaged shared library. That library contains a project-owned C ABI shim, the pinned Claricle resvg v0.48.1 fork, libpng 1.6.59, and zlib 1.3.2. The shim owns the premultiplied-RGBA-to-straight-RGBA conversion and the fixed PNG encoding profile. On every other target, RubyGems can select a same-version `ruby`-platform shell gem containing the identical Ruby API but no native library; installation and non-PNG Vectory use remain available, while the first PNG render raises `Svgpng::UnsupportedPlatformError`. Neither package compiles at installation time.

`Vectory::Svg#to_png` rejects DTD-bearing raw input before Vectory's existing Leptris parse/reserialize step, passes its serialized `Svg#content` to the public renderer boundary, and returns a binary `Vectory::Png` whose warnings remain observable. `Vectory::Emf`, `Vectory::Eps`, and `Vectory::Ps` expose the same PNG projection by delegating through their existing `to_svg` conversion. Vectory declares `svgpng ~> 0.1.0`; integration and review use the checksummed, unpublished 0.1.0 native package for the development/CI platform after the public API and error contract are complete. The CLI accepts explicit font, fallback, and render-profile options for PNG output and maps every `Svgpng` failure into the existing Vectory exit-status families. The declared CI matrix renders a pinned corpus and byte-compares the output before the exact tested seven native packages and `ruby`-platform shell package are published together, then validates representative Vectory output through a commit-pinned `png_conform` test dependency.

## Technical decisions

- **Placement:** `claricle/svgpng` is a new sibling gem to `emfsvg` and `postsvg`. Cards 01–03 and 05 are implemented in that repository, and every path in those cards is relative to its root. Cards 04 and 06 are Vectory integration work.
- **Engine patch:** use a Claricle fork based exactly on resvg v0.48.1. Its narrow C ABI patch exposes the computed CSS family/style/weight/stretch request, selected face ID, and missing-glyph diagnostics so the supplied font chain can be enforced. A parser-side policy rejects any reference other than a same-document fragment or inline `data:` value before resvg can load it; it does not expose a resource callback or accept resource bytes.
- **PNG encoder:** the project-owned shim statically links libpng 1.6.59 and zlib 1.3.2. It converts resvg's premultiplied RGBA8 output to straight RGBA8 with a specified integer rule, then emits a fixed 8-bit RGBA, non-interlaced PNG with fixed filtering, compression, chunk buffering, and no ancillary metadata.
- **Binding and packaging:** use Ruby's Fiddle to load only the packaged shim. Build seven precompiled native gems and one API-compatible `ruby`-platform shell gem at version 0.1.0, with no compiler or Rust toolchain required at install time and no Ruby-version-specific native artifact dimension. The shell permits dependency resolution on unsupported targets and raises `Svgpng::UnsupportedPlatformError` only from rendering; Card 05 publishes the already-tested eight-package set after the seven native byte gates pass.
- **Initial byte-identical matrix:** `arm64-darwin`, `x86_64-darwin`, `x86_64-linux`, `aarch64-linux`, `x86_64-linux-musl`, `aarch64-linux-musl`, and `x64-mingw-ucrt`. Add later platforms as extra CI jobs only after they pass the same corpus.
- **Fonts:** disable system-font discovery. Resolve exact normalized families from `fonts:`, then ordered `fallback_fonts:` entries, with face metadata and glyph coverage derived from the supplied bytes. A substitution returns a warning naming the requested and substitute families; exhausting both inputs raises a clear error naming the missing family and glyphs. Bundle no fonts.
- **External references:** support only self-contained SVG: reject every raw XML `DOCTYPE` before Vectory or `svgpng` constructs a DOM, allow same-document fragments and inline `data:` values, and reject relative paths, absolute paths, network URLs, protocol-relative URLs, `file:` URLs, CSS imports, and every other non-inline reference before rendering. Do not set resvg's filesystem resources directory or provide any filesystem/network loader.
- **Limits:** enforce only deterministic preflight/pre-allocation bounds on SVG bytes, font count and bytes, output dimensions, pixel count, and row/output buffer arithmetic. Do not claim an in-process wall-clock deadline that resvg's synchronous C API cannot cancel safely.
- **Conformance dependency:** pin `png_conform` 0.1.4 at commit `653abae0d16d9533a37a5d5eca40896d3b307993` as a Vectory test-group Git dependency and require its validation service explicitly.

## Rationale and measured evidence

| Option | Support and determinism evidence | Installation footprint evidence | Licence | Existing Claricle usage | Outcome |
| --- | --- | --- | --- | --- | --- |
| **Sibling `claricle/svgpng` with owned shim, patched resvg v0.48.1, libpng 1.6.59, and zlib 1.3.2** | resvg is a static-SVG renderer with caller-loaded fonts, but its C API yields premultiplied RGBA and lacks a selection callback. The narrow fork supplies font enforcement/diagnostics; the shim owns straight-alpha conversion and byte-stable encoding. Seven-platform goldens test the combined closure. | Upstream's sub-5 MB CLI is only directional. Installed gem/library size is measured for each built artifact. Static encoder dependencies keep the runtime closure to one Fiddle-loaded file and avoid Ruby-version multiplication. | resvg Apache-2.0 OR MIT; libpng license; zlib license; `svgpng` project license to be recorded at repository creation | `emf` provides the Fiddle-loading precedent; `emfsvg` and `postsvg` provide the sibling projection-gem boundary. No SVG rasterizer or PNG encoder is currently used. | Selected: the smallest closure that actually owns font choice and PNG bytes. |
| Put the native wrapper inside `vectory` | Same rendering properties as the selected engine. | Same native release matrix, but every Vectory install would carry native release mechanics and payload. | Same native licences plus Vectory BSD-2-Clause | No existing renderer usage. | Rejected: mixes native artifact responsibilities into Vectory and prevents reuse by sibling tools. |
| `rsvg2` / librsvg | Broad SVG support and mature text layout, but its normal text stack includes Pango, HarfBuzz, FreeType, and Fontconfig, enlarging the state that must be pinned for byte determinism. | The published Ruby binding is about 29 KB but excludes its native stack. Official build requirements include C, Rust/Cargo, Meson, Cairo, FreeType, GLib, libxml2, and Pango; the full installed closure is unmeasured. | LGPL-2.1-or-later | None found. | Rejected: larger native closure and system integration burden. |
| `ruby-vips` / libvips | The Ruby API can rasterize SVG, but libvips delegates SVG loading to librsvg or ImageMagick according to its build, introducing backend variance. | The published Ruby binding is about 72.5 KB and excludes libvips and its SVG backend; the full installed closure is unmeasured. | ruby-vips MIT; libvips LGPL-2.1-or-later | None found. | Rejected: broad image-processing dependency and backend variability. |

Sources: [resvg repository and licence](https://github.com/linebender/resvg), [resvg C API](https://github.com/linebender/resvg/blob/main/crates/c-api/README.md), [resvg v0.48.1 release](https://github.com/linebender/resvg/releases/tag/v0.48.1), [resvg unsupported features](https://github.com/linebender/resvg/blob/main/docs/unsupported.md), [libpng 1.6.59 release and checksums](https://www.libpng.org/pub/png/libpng.html), [zlib 1.3.2 release](https://zlib.net/), [rsvg2 package](https://rubygems.org/gems/rsvg2), [librsvg build requirements](https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/compiling.html), [ruby-vips package](https://rubygems.org/gems/ruby-vips), and [libvips](https://github.com/libvips/libvips).

## Ordering and effort

| Order | Card | Repository | Effort | Dependency |
| --- | --- | --- | ---: | --- |
| 1 | [01 — Codify the complete rendering contract](01-lock-rendering-architecture.md) | `claricle/svgpng` | 2–3 engineer-days | New repository available |
| 2 | [02 — Package the owned renderer and PNG encoder](02-package-owned-renderer.md) | `claricle/svgpng` | 6–9 engineer-days | Card 01 |
| 3 | [03 — Enforce deterministic fonts and external-reference rejection](03-enforce-font-determinism.md) | `claricle/svgpng` | 4–6 engineer-days | Card 01; native enforcement tests use Card 02 |
| 4 | [04 — Add PNG output and inputs to Vectory](04-integrate-vectory-png-output.md) | `claricle/vectory` | 3–5 engineer-days | Cards 02–03; checksummed unpublished `svgpng` 0.1.0 native artifact for the development/CI platform |
| 5 | [05 — Pin cross-platform golden output](05-pin-cross-platform-goldens.md) | `claricle/svgpng` | 3–5 engineer-days | Cards 02–03; may run in parallel with Card 04 |
| 6 | [06 — Document and prove the conformance handoff](06-document-conformance-handoff.md) | `claricle/vectory` | 1–2 engineer-days | Cards 04–05 |

Base work totals **19–30 engineer-days**. Reserve a separate **3–5 engineer-day contingency** only if the seven-platform build exposes signing, loader, or cross-toolchain work not observable before artifacts are built. If that contingency is triggered, total work is **22–35 engineer-days**; across the base and contingency-triggered scenarios, the overall possible envelope is **19–35 engineer-days**.

## Decisions still needed

None. Placement, engine/fork, Fiddle packaging, the seven-platform matrix, font policy, and the self-contained-SVG boundary are fixed above.

## Relationship to claricle/vectory#77

Issue #77 changes SVG composition and selector/identifier transforms; issue #76 consumes the final serialized SVG. Neither issue should depend on the other's internals. Its parallel plan identifies direct edit overlap at `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc`; its transform fixtures/specs are otherwise separate from this plan's raster fixtures/specs. The second branch to land should rebase, reconcile those four files, keep “transform pipeline” terminology distinct from rendering/projection terminology, and rerun both sets of targeted checks.
