# 06 — Document and prove the conformance handoff

## Work

This card is implemented in `claricle/vectory`; all paths are relative to that repository. Add this test-only Git dependency to the tracked Vectory `Gemfile`:

```ruby
gem "png_conform",
    git: "https://github.com/claricle/png_conform.git",
    ref: "653abae0d16d9533a37a5d5eca40896d3b307993",
    group: :test
```

That exact Git ref is the repository-visible pin and reports version 0.1.4. Preserve Vectory's existing policy: `Gemfile.lock` remains ignored and untracked, and no Done-when check depends on a developer's local lockfile. Do not add `png_conform` to `vectory.gemspec` or Vectory's runtime dependencies. Bundler supplies the pinned checkout's `lib/` directory; `spec/integration/png_conform_spec.rb` must explicitly `require "png_conform/services/validation_orchestrator"` rather than relying on an incidental top-level require.

Render a representative SVG with explicit font inputs, write the returned `Vectory::Png` to a temporary file, and pass that path directly to `PngConform::Services::ValidationOrchestrator.validate_file`. Assert the returned analysis is `valid?`; when it is not, make the assertion output include its `errors`/serialized diagnostics. Add a deliberately corrupted temporary copy as a negative control and assert that its returned analysis is not `valid?` and contains at least one diagnostic error. This proves the existing file boundary and useful failure reporting without adding a bespoke adapter.

Update `README.adoc` and `docs/reference/api.adoc` with reproducible Ruby and CLI calls, all PNG-only flags, profile precedence, exact and fallback font behavior, warning presentation, raw-DTD and non-inline-reference rejection, the seven native platforms, the unsupported-platform shell behavior, deterministic-byte boundary, enforceable limits, wall-clock isolation guidance, common errors/statuses, and the `png_conform` handoff. Link to `svgpng`'s support, provenance, encoder, golden-compatibility, and release documentation rather than duplicating native details in Vectory.

Before landing, compare the final #77 branch. Its plan identifies shared edits in `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc`; reconcile those files without making SVG-to-PNG depend on phase-two transform internals. Keep its “transform pipeline” name distinct from rendering/projection terminology. The contract remains: #77 may produce the final SVG bytes; #76 consumes those bytes.

## Done when

- The tracked `Gemfile` pins `png_conform` 0.1.4 to commit `653abae0d16d9533a37a5d5eca40896d3b307993` in the test group, with no runtime gemspec dependency; `Gemfile.lock` remains ignored and untracked under existing repository policy.
- The integration spec explicitly loads `PngConform::Services::ValidationOrchestrator` through Bundler and hands it the written `Vectory::Png` path without an adapter.
- The successful analysis is asserted through `valid?` with diagnostic output on failure, and a corrupted-file control asserts `valid? == false` plus nonempty errors.
- Public documentation gives reproducible Ruby and CLI examples for fonts, fallbacks, profiles, warnings, DTD/external-reference rejection, supported targets, unsupported-platform installation/render behavior, limits, error statuses, and conformance.
- The final #77 overlap check is recorded, shared-file conflicts are reconciled, and neither feature depends on the other's internal classes.

```sh
set -e
rg -U 'gem "png_conform",\n\s+git: "https://github\.com/claricle/png_conform\.git",\n\s+ref: "653abae0d16d9533a37a5d5eca40896d3b307993",\n\s+group: :test' Gemfile
git check-ignore -q Gemfile.lock
bundle exec ruby -e 's = Gem::Specification.load("vectory.gemspec"); abort if s.runtime_dependencies.any? { |dependency| dependency.name == "png_conform" }'
bundle exec rspec spec/integration/png_conform_spec.rb
```

## Status
State: todo
PR: none

## Effort

1–2 engineer-days.
