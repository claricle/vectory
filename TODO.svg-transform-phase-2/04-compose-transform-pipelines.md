# 04 — Compose named transform pipelines

Effort: **12–18 engineer-hours**.

## Purpose

Represent grafting and the unary SVG cleanup steps as inspectable transform values while exposing only the ordered whole-document and per-SVG profiles required by issue #77 and the verified Standoc boundary.

## Changes

- Add `lib/vectory/svg_transform.rb` with factories for `graft_into(host_content:)`, `disambiguate(index:, duplicate_ids:)`, `suffix_classes(index:)`, and a namespace step whose arguments exactly match Card 02's accepted contract record.
- Act as the Cards 01–03 shared-file integrator: add the three primitive autoloads plus the transform and pipeline autoloads to `lib/vectory.rb` together after the primitive files are ready. Behavioral work for Cards 01–03 does not concurrently own this table.
- Make `graft_into(host_content:)` capture the original serialized host and expose the stable step name `graft_into`. Its call delegates to `SvgSubtreeGraft.replace_root_content(host_content, processed_content)`, making the binary primitive unary at execution without concealing either input. Validate the captured host when creating the step and record whether its document element is `svg` for scope checking.
- Add `lib/vectory/svg_transform_pipeline.rb` for immutable step composition with `>>`, `#call(content)`, and an inspectable `#steps` collection whose entries expose stable names and deeply frozen option snapshots.
- Adapt `SvgIdDisambiguator.rewrite` through the `disambiguate` step rather than copying its logic. Derive the existing `_inject_<index>` suffix consistently for both ID and class steps.
- Make `disambiguate(index:, duplicate_ids:)` defensively copy and deeply freeze the supplied collision-map snapshot. Pipeline execution reads that snapshot and never mutates the caller's map or captured nested values.
- Expose exactly three executable named profiles: `graft` (`G`) for one whole-document graft, `svg_cleanup` (`D -> S -> N`) for one selected SVG, and `graft_then_svg_cleanup` (`G -> D -> S -> N`) for a captured SVG-root host. The complete four-step profile proves that the graft primitive composes with the other transforms; it is not the Standoc complete-host migration flow.
- Let `>>` accumulate only canonical prefixes that can become one of those profiles. Reject skipped, duplicate, reversed, and non-leading graft steps when the invalid append occurs; reject calling an incomplete prefix such as `D`, `D -> S`, `G -> D`, or `G -> D -> S`. Do not publish the other order-preserving subsequences as executable pipelines.
- Define the three execution scopes explicitly: `G` accepts any rooted processed serialization and returns the captured complete host with replaced root content; `D -> S -> N` accepts exactly one serialized SVG root; `G -> D -> S -> N` accepts a processed serialization only when the captured host document element is `svg`. Nested SVG descendants stay within the selected SVG unit and share its index.
- For multiple embedded SVGs, run a `G` pipeline once on the complete mapped document. Then, for each selected SVG in document order, construct a fresh `D -> S -> N` pipeline using that SVG's positional index and an immutable snapshot of the caller's current collision map; replace the SVG, merge its post-rewrite IDs into the caller-owned map, and only then construct the next pipeline. Do not reuse a pipeline across evolving map states and do not substitute `SvgIdDisambiguator.duplicate_ids` for the Standoc compatibility path.
- Return new pipeline values from `>>`; do not mutate either operand. Define stable `ArgumentError` messages that identify an invalid step sequence, incomplete profile, or execution-scope mismatch.
- Ensure every adapter uses a serialized-string boundary and propagates primitive parse/validation errors without partially advancing a pipeline.
- Add `spec/vectory/svg_transform_pipeline_spec.rb` for step metadata, host capture, deep snapshot immutability, the three profile names and exact step sequences, canonical-prefix construction, invalid/incomplete order handling, scope rejection, error propagation, ID/class suffix consistency, and idempotence of all three executable profiles.
- Include one selected-SVG `G -> D -> S -> N` example that preserves captured host-root metadata and applies the namespace contract selected in Card 02 after grafting. Include a complete host with two embedded SVGs where `G` succeeds alone and fresh per-SVG `D -> S -> N` pipelines are built from successive collision-map snapshots. Repeated namespaced root IDs must leave the first root unchanged and give the later root its positional suffix. Test Vectory's batch `SvgIdDisambiguator.duplicate_ids` path separately from the Standoc compatibility oracle.

## Dependencies

- Cards 01–03, including Card 02's accepted namespace contract record.
- The existing `SvgIdDisambiguator` API remains unchanged.

## Done when

- Named transform values compose with `>>`, and pipeline steps expose stable names plus deeply frozen option snapshots.
- The only executable public profiles are `G`, `D -> S -> N`, and `G -> D -> S -> N`, with stable profile names and execution scopes.
- The graft step captures its host and delegates processed input to `SvgSubtreeGraft` without changing the primitive's two-input semantics.
- `G` accepts a complete processed host and preserves the captured host root.
- `D -> S -> N` accepts exactly one selected serialized SVG and rejects a surrounding host with sibling SVGs.
- `G -> D -> S -> N` executes only for a captured SVG-root host and preserves host-root metadata while retaining namespace changes applied after grafting.
- Canonical construction prefixes can be extended to a supported profile, but an incomplete prefix cannot be called.
- Skipped, duplicate, reversed, and non-leading graft sequences are rejected before XML processing begins.
- The disambiguation adapter delegates to `SvgIdDisambiguator` and shares the `_inject_<index>` suffix with class suffixing.
- Constructing a pipeline defensively copies and deeply freezes the supplied duplicate-ID snapshot; later caller mutations do not change that pipeline and pipeline execution does not change caller state.
- The two-SVG compatibility example builds a fresh `D -> S -> N` pipeline from the current snapshot for each SVG, merges IDs between constructions, leaves the first repeated namespaced root ID unchanged, and gives the later root its positional suffix.
- Reapplying each of the three executable profiles with the same immutable inputs produces XML-equivalent output.
- The focused disambiguator and pipeline specs pass.

Planned checks (not yet run):

```sh
bundle exec rspec spec/vectory/svg_id_disambiguator_spec.rb spec/vectory/svg_transform_pipeline_spec.rb
```

## Status
State: implemented; publication, required lint and CI pending
PR: not yet published (GitHub access unavailable)
