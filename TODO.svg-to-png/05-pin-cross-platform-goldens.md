# 05 — Pin cross-platform golden output

## Work

This card is implemented in `claricle/svgpng`; all paths are relative to that repository. Create a small, reviewable, licence-clean corpus under `spec/fixtures/svg_to_png/` covering dimensions/viewBox, paths and strokes, gradients/clip/mask, alpha/background, all alpha-unpremultiplication edge cases, inline raster data, same-document fragments, exact font faces, style/weight/stretch matching, ordered fallback and warning order, partial glyph coverage, missing fonts/glyphs, non-inline external-reference rejection, and each enforceable pre-allocation limit. Include representative static constructs resvg does not implement only to pin its documented output behavior; do not assert that every unsupported construct raises an error.

Record source, licence, complete profile, SVG/font digests, normalized font metadata, resvg fork/upstream/patch identifiers, libpng/zlib/toolchain/build identifiers, PNG encoding settings, expected dimensions, ordered warnings/errors, and SHA-256 in a manifest. Golden PNG inspection must assert 8-bit RGBA, non-interlaced encoding, `IHDR`/`IDAT`/`IEND` ordering, and absence of ancillary chunks as well as the whole-file digest.

Add `spec/svgpng/golden_spec.rb` to render each positive fixture twice in a clean process, compare exact bytes and digest to the committed PNG, inspect the encoded structure, and check stable errors only for malformed input, font/glyph resolution, forbidden external references, profile/limit, loader/native, and encoder negative fixtures. Run that exact corpus on `arm64-darwin`, `x86_64-darwin`, `x86_64-linux`, `aarch64-linux`, `x86_64-linux-musl`, `aarch64-linux-musl`, and `x64-mingw-ucrt` in `.github/workflows/golden.yml`. No 0.1.0 gem is published before all seven jobs match every committed byte. Card 04 develops and reviews against Card 02's checksummed local development-platform package while this gate runs.

After the gate passes, verify that the seven native gems and `ruby`-platform shell gem are byte-for-byte the staged Card 02 release candidates by checking their recorded SHA-256 values; do not rebuild them. Publish those exact eight immutable 0.1.0 files as one release set, then run clean source-resolution checks on a supported target and an unsupported target. The supported target must select its native gem and reproduce the minimal golden; the unsupported target must select the shell, require the API, and raise `Svgpng::UnsupportedPlatformError` only on render. Only then may Card 04 merge or Vectory release/advertise PNG output. A target enters the advertised support table only after its job matches every committed byte. Later platforms are additive CI jobs subject to the same gate.

Add `svgpng:verify_release_set` in the proposed `rakelib/release.rake`. Before publication it compares the eight staged files with the Card 02 checksum manifest and refuses a missing, additional, or changed artifact; after publication it downloads each platform variant from the configured gem source, verifies the same digests, and drives the supported/unsupported clean-resolution smoke jobs. Keep credentials and the mutating publish action outside ordinary test runs; this verification task is read-only and repeatable.

Add `svgpng:verify_manifest` and an explicit golden-update task in `rakelib/golden.rake`. The update task prints old/new digests and the resvg-fork, encoder, and toolchain changes that produced them, and requires an intentional compatibility update; ordinary tests never rewrite fixtures. Keep the corpus bounded rather than introducing a corpus sweep or fuzzer.

> Versioning note: until 1.0, a golden PNG byte change requires a minor version bump and CHANGELOG entry; from 1.0 onward it requires a major version bump. This is the working rule pending confirmation and does not block implementation.

## Done when

- Every fixture records provenance, licence, complete inputs, font digests, renderer/fork/encoder/toolchain identifiers, PNG settings, expected dimensions/warnings, and expected SHA-256.
- Golden structure tests prove the fixed RGBA conversion, bit depth, colour type, filter/compression behavior, chunk order, and metadata omission in addition to whole-file bytes.
- All seven initial platforms render every positive fixture twice to bytes identical to the committed golden PNG.
- Host-font, locale, timezone, working-directory, environment, render concurrency, and clean-process perturbations do not alter golden bytes or warnings.
- Font fallback/partial-coverage fixtures produce the same selected-face diagnostics, while only enforceable input/forbidden-reference/limit/native/encoder failures assert stable errors.
- CI fails when any target omits the corpus or produces different bytes, not merely a visually similar image.
- No version 0.1.0 package is published until all seven native targets pass; publication uses the recorded checksums to prove the exact staged seven native gems and one `ruby` shell gem were published without rebuilding.
- The release-set verification task proves that all eight published artifacts match the staged checksums without rebuilding; post-publication clean resolution selects and renders with a native gem on a supported target, while an unsupported target installs/requires the shell and raises the documented error only when rendering.
- The explicit golden-update command reports old/new digests and dependency/toolchain compatibility impact and cannot run during ordinary tests.

```sh
bundle exec rspec spec/svgpng/golden_spec.rb && bundle exec rake svgpng:verify_manifest svgpng:verify_release_set
```

## Status
State: todo
PR: none

## Effort

3–5 engineer-days.
