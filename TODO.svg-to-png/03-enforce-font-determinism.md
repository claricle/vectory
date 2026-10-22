# 03 — Enforce deterministic fonts and external-reference rejection

## Work

Implement the fixed input policy in `claricle/svgpng`, using the proposed `lib/svgpng/font_face.rb`, `lib/svgpng/font_catalog.rb`, and Card 01's `lib/svgpng/render_profile.rb`, the parser-side policy in the proposed `native/resvg-patches/reject_external_references.patch`, and focused specs under `spec/`. Do not redefine Card 01's sizing, validation, rounding, or background rules here. If the new repository adopts different names, record the final paths in `docs/decisions/svg-to-png.md`. Disable resvg system-font loading and bundle no fonts.

### Font resolution

`fonts:` maps a caller spelling of a requested CSS family to one or more `Svgpng::FontFace(bytes:, source_id:)` values. `fallback_fonts:` is an ordered array of the same value. Parse OpenType metadata and coverage from the bytes through the pinned native stack; do not trust caller declarations for style, weight, stretch, or glyph coverage. Normalize family matching by Unicode NFC, CSS quote removal and whitespace collapse, and ASCII case folding. Validate that every `fonts:` key matches at least one family name in the supplied face metadata. Reject invalid fonts and duplicate faces with the same normalized family/style/weight/stretch as deterministic input errors instead of choosing by hash or filesystem order.

Use resvg's computed CSS family/style/weight/stretch request and the Card 02 fork callback, so the project resolver chooses the face that resvg actually uses for shaping and rendering. Within an exact requested family, use the pinned resvg/fontdb CSS face-matching algorithm. Resolve missing glyphs per shaped cluster: try matching exact-family faces with full cluster coverage, then `fallback_fonts:` strictly in caller order while choosing style/weight/stretch within each fallback family. The callback supplies the chosen loaded face ID back to resvg; it is not a disconnected preflight guess.

Emit substitution warnings in first document-use order, deduplicated by requested family, selected family, style/weight/stretch, and missing Unicode scalar set. Each warning names both families and the triggering scalars. If no supplied face covers a cluster, raise a stable missing-glyph error naming the requested family and scalars. Expose the resolved face/source digest manifest as diagnostics. Specs cover quoted/case variants, multiple faces, duplicate metadata, style/weight/stretch matching, partial glyph coverage, multi-codepoint clusters, fallback order, deterministic warning order, and missing glyphs.

### External-reference policy

Keep the public boundary self-contained: it accepts no resource map, directory, base path, or loader. Permit same-document fragments such as `href="#symbol"` and `url(#gradient)`, plus inline `data:` values whose bytes already belong to the SVG input. At the raw `Svgpng.render` byte boundary, reject any XML `DOCTYPE` declaration before DOM construction or entity expansion; this deliberately conservative rule prevents both external entities and entity-expansion variants without performing a secondary read. The pinned XML/CSS parser then rejects relative paths, absolute and drive/UNC paths, protocol-relative URLs, `file:`, `http:`, `https:`, every other non-`data:` scheme, and CSS `@import` before rendering. Apply the same rule to every URI-bearing SVG attribute and CSS `url(...)`; malformed URI syntax is an input error rather than a best-effort load.

The DTD guard is token-aware so `DOCTYPE` text inside comments or CDATA is not misclassified; the remaining rejection is enforced at the fork's parser/load boundary, after XML and CSS tokenization but before any resolver or decoder asks for external bytes. It returns the reference class and document location through a stable forbidden-reference status without echoing sensitive host content. Do not set resvg's filesystem resources directory, install a resource callback, change the process working directory, materialize files, or provide an HTTP client. Tests instrument file and socket entry points and prove that DTD-bearing and URI-bearing inputs fail without a filesystem or network attempt, including under concurrent renders and with symlink/traversal-looking inputs. Inline `data:` raster fixtures and same-document fragment fixtures remain positive controls.

Apply only the fixed SVG/font counts and byte limits plus requested dimension, pixel-count, row-stride, and output-buffer arithmetic checks before the corresponding allocation. There is no in-process render-time deadline or broad promise to reject every SVG feature unsupported by resvg. Document that callers needing a wall-clock limit must isolate rendering in a killable process.

## Done when

- System fonts are disabled and the packaged gem contains no bundled font files.
- Normalized exact-family and face-attribute matching is derived from supplied font metadata, rejects ambiguous duplicates, and takes precedence over ordered fallback entries.
- The patched callback makes the selected face the one resvg shapes/renders and reports deterministic substitution or missing-glyph diagnostics for partial coverage.
- The public API exposes no resource injection surface; same-document fragments and inline `data:` values render, while any raw XML `DOCTYPE` and every non-inline SVG/CSS reference produce the stable forbidden-reference error before entity expansion or secondary I/O.
- Relative, absolute, traversing, symlink-looking, filesystem, protocol-relative, network, imported-CSS, and other-scheme references fail before any filesystem or socket attempt, including during concurrent renders.
- Only documented byte/count/dimension/allocation limits produce stable limit errors; no unimplementable in-process timeout or blanket unsupported-feature error is claimed.
- Tests perturb host fonts, locale, timezone, working directory, environment variables, and concurrent render order without changing produced bytes or warnings.

```sh
bundle exec rspec spec/font_catalog_spec.rb spec/external_reference_policy_spec.rb spec/render_profile_spec.rb
```

## Status
State: todo
PR: none

## Effort

4–6 engineer-days.
