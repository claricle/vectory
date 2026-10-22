# 02 — Package the owned renderer and PNG encoder

## Work

Implement the native layer in `claricle/svgpng`. The proposed `native/svgpng_shim/` sources own one stable C ABI for the Fiddle boundary. Pin a Claricle resvg fork commit whose upstream base is exactly v0.48.1, libpng 1.6.59, zlib 1.3.2, the Rust/C toolchains, build flags, source checksums, licences, and patch digests in `vendor/manifest.yml`. Limit the resvg delta to the font-selection/diagnostic hook described in Card 03 and a parser-side failure for forbidden external references; keep each change reviewable as a discrete patch and add tests proving the upstream C API cannot enforce the required font chain. The rejection patch accepts no resource bytes and exposes no loader callback.

The shim must load caller font bytes, expose the font resolver callback plus diagnostic events, render through resvg into premultiplied RGBA8, unpremultiply into a separately owned straight-RGBA buffer with Card 01's checked integer formula, and encode that buffer through the statically linked libpng/zlib versions. It owns all native allocation/free functions and maps parse, resolver, forbidden-reference, allocation/overflow, render, and encoder failures to versioned status codes; Ruby never frees memory with a different allocator.

Configure libpng for colour type 6, 8-bit depth, no interlace, `PNG_FILTER_NONE`, zlib level 9/default strategy/window 15/memory level 8, and a 32 KiB compression buffer. Do not write ancillary chunks. Unit-test alpha-zero normalization, rounding at low alpha, opaque pixels, channel clamping, row order, overflow guards, and exact chunk order (`IHDR`, one-or-more `IDAT`, `IEND`). The single artifact is the shim with the resvg fork, libpng, and zlib linked into it; neither Fiddle nor the dynamic loader may resolve a system copy of those dependencies.

Bind only that C ABI through `lib/svgpng/native.rb` and keep orchestration in `lib/svgpng/renderer.rb`. Do not add a compiled Ruby extension, shell out, search system library paths, or compile at gem installation time.

Build, but do not publish, the initial release candidate as version 0.1.0 for exactly `arm64-darwin`, `x86_64-darwin`, `x86_64-linux`, `aarch64-linux`, `x86_64-linux-musl`, `aarch64-linux-musl`, and `x64-mingw-ucrt`. One shared-library artifact per platform must serve every supported Ruby version. Also build an eighth, same-version `ruby`-platform shell gem containing the public Ruby API and errors but no native library. RubyGems must prefer the exact native package on a supported target when the complete local package set is offered; on any other target it can resolve, install, and require the shell, and only a render call raises `Svgpng::UnsupportedPlatformError` with the detected platform and supported-target list. Non-render API loading must not probe ambient libraries.

Add build logic in `svgpng.gemspec`, `rakelib/native.rake`, and `.github/workflows/native.yml`; add loader, ABI, encoder, package-resolution, and smoke specs under `spec/`. Stage the seven native gems, the shell gem, checksums, and provenance as one immutable release-candidate set. Card 04 consumes the checksummed local native package for Vectory's development/CI target. Card 05 owns the byte-identity gate and publishes these exact eight tested files only after all seven native targets pass; Card 02 must neither publish a partial platform set nor rebuild version 0.1.0 after that gate.

Each native build emits the source and binary checksums, resvg fork/upstream/patch identifiers, libpng/zlib versions, Rust/C compiler and linker identifiers, flags, licence notices, and installed gem/library size; the shell manifest records that it contains no native payload. In clean environments without Rust, a C compiler, libpng, or zlib, install the matching native package on a supported target and prove a minimal render succeeds. On an unsupported target, install the `ruby` package and prove `require "svgpng"` plus non-render API inspection succeeds while rendering raises the documented unsupported-platform error. Run resolution checks against a local index containing all eight packages so package selection, rather than a hand-picked gem install, is under test.

## Done when

- The provenance manifest pins the resvg fork and v0.48.1 base, the font hook and external-reference rejection patches, libpng 1.6.59, zlib 1.3.2, source checksums, toolchains, flags, and all licences.
- Native tests prove that the selected loaded font face is used for shaping/rendering and that relative, absolute, filesystem, network, protocol-relative, imported-CSS, and other non-inline references fail before rendering while resvg's filesystem resources directory remains unset.
- Native tests prove the exact unpremultiplication rule, buffer ownership, overflow handling, PNG profile, chunk ordering, and error-status mapping.
- The packaged artifact links the renderer and encoder closure into one shared library and does not resolve ambient resvg, libpng, or zlib libraries.
- The build workflow produces an unpublished, checksummed 0.1.0 release-candidate set containing one shared-library gem for each of the seven initial targets plus one native-free `ruby`-platform shell gem; it performs no gem publication.
- A checksummed, installable `svgpng` 0.1.0 native gem is produced for the Vectory development/CI platform after the Card 01 API/error contract and Card 03 policy are finalized, providing Card 04's measurable local integration prerequisite.
- A supported clean environment resolves the matching native gem from the complete local package set and renders without Rust, a C compiler, system encoder libraries, or a Ruby-version-specific build.
- An unsupported clean environment resolves the `ruby` shell from that same set, installs and requires it successfully, and reaches `Svgpng::UnsupportedPlatformError` only when rendering, so loading Vectory for non-PNG work remains possible.
- The Fiddle loader opens only the gem-packaged library on a supported platform and neither the native nor shell package searches ambient paths.
- The packaged interface renders a minimal SVG byte string to deterministic valid PNG bytes without shelling out.

```sh
bundle exec rspec spec/native_spec.rb spec/encoder_spec.rb spec/renderer_spec.rb spec/package_resolution_spec.rb && bundle exec rake native:verify_artifacts
```

## Status
State: todo
PR: none

## Effort

6–9 engineer-days.
