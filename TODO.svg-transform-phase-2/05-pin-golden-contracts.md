# 05 — Pin golden contracts and documentation

Effort: **14–20 engineer-hours**.

## Purpose

Prove the combined public contract on representative embedded SVG content and give Standoc #1298 a precise migration target without changing the consumer in this issue.

## Changes

- Add `spec/fixtures/svg_transform/pipeline_profiles.yml` that enumerates only the three executable public profiles: whole-document `G`, per-SVG `D -> S -> N`, and selected-SVG `G -> D -> S -> N`. Map each stable profile name to its step sequence, input, optional captured host, immutable options, and checked-in expected output.
- Add one expected-output golden per profile under `spec/fixtures/svg_transform/pipeline_outputs/`. The `G` output proves the graft boundary, the `D -> S -> N` output makes IDs, class lists, compound and nested selectors, and the accepted namespace operation observable, and the `G -> D -> S -> N` output proves all four primitives compose on an SVG-root host without losing later root-level changes. Cards 01–03 retain separate primitive goldens; do not create public singleton or arbitrary-subsequence pipeline profiles merely to duplicate them.
- Add an original complete host with two sibling embedded SVGs, its complete `SvgMapping` result, and the final expected complete host under `spec/fixtures/svg_transform/consumer_flow/`. The fixture begins at the same boundary as Standoc's `svgmap_cleanup`: map the complete prepared host with `Vectory::SvgMapping`, pass that complete processed document through one `G`-only pipeline that captured the original host, then select the embedded SVG roots from the grafted document.
- In that consumer flow, preserve Standoc's sequential collision contract: start with its caller-supplied collision map; before each selected SVG, defensively snapshot the current map and construct a fresh `D -> S -> N` pipeline with positional index `0` or `1`; replace the SVG; then merge its post-rewrite IDs into the caller-owned map before constructing the next pipeline. Give both selected SVG roots the same namespaced ID in the input; assert the first remains unchanged and the later root becomes `<id>_inject_1`. Assert that each pipeline retains its original frozen snapshot and never observes later map updates. Do not derive this compatibility map with `SvgIdDisambiguator.duplicate_ids`, which would suffix both roots. Keep a separate fixture/example for Vectory's batch `duplicate_ids` behavior, and keep the selected-SVG `G -> D -> S -> N` fixture separate from Standoc's complete-host boundary.
- Add `spec/vectory/svg_transform_integration_spec.rb` that verifies the manifest and public executable-profile set are identical, builds the three named profiles, compares each result with its golden, reparses it, verifies step-relevant host content/references/classes/namespaces, and proves every profile idempotent. Compare the complete-host consumer flow with its golden, prove repeating that full orchestration is XML-equivalent, and assert the fresh-snapshot first/later root behavior, retained host-root metadata, one whole-document graft, and final namespace declarations. Test Vectory's batch `duplicate_ids` result separately so it cannot become the Standoc migration oracle.
- Extend `README.adoc` with the supported step order, a concise `>>` example, the secure Crass requirement, string boundary, idempotence guarantee, and the distinction between namespace binding and legacy `SvgDocument#namespace`.
- Extend `docs/reference/api.adoc` with each public class/method, the namespace signature and error behavior selected in Card 02's contract record, the three pipeline profiles, return types, and a Standoc-oriented migration example based on the checked-out `svg_cleanup`/`svgmap_cleanup` sequence. Show a fresh per-SVG pipeline built from each collision-map snapshot. Show #1298's `SvgIdDisambiguator` consumption and describe `SvgClassSuffixer` as the canonical Vectory implementation available to replace the class suffixing currently in `lib/metanorma/cleanup/image.rb` lines 171–185.
- Keep #1298's scope explicit: it adopts ID disambiguation while retaining its Crass helpers. Before the #77 handoff closes, create or identify and record in `90-progress.md` the URL of a downstream task named **Adopt `Vectory::SvgClassSuffixer` in Standoc**; that task depends on a #77-containing Vectory release and owns later adoption and removal of the inline helpers.
- Land this issue first. Record a handoff for #76's later Vectory integration to rebase and reconcile `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc`, keeping raster projection terminology distinct from content transforms.
- Build the documentation site with the repository's production Jekyll command so malformed AsciiDoc or site configuration fails before handoff.

## Dependencies

- Cards 03–04.
- Read-only validation against Standoc cleanup behavior and the confirmed #1298 `SvgIdDisambiguator` handoff.

## Done when

- The manifest enumerates exactly the three executable public profiles `G`, `D -> S -> N`, and `G -> D -> S -> N`, with no arbitrary subsequence profiles.
- Each public primitive matches the focused golden owned by Cards 01–03 or the existing `SvgIdDisambiguator` specs.
- Each of the three named pipeline profiles matches its checked-in expected-output golden.
- Every golden output reparses successfully.
- No fixture passes a surrounding host document containing sibling SVGs to a pipeline that includes `D`, `S`, or `N`; only the `G` profile receives the complete processed host.
- The selected-SVG `G -> D -> S -> N` golden proves grafting preserves captured host-root metadata and later namespace binding retains requested final declarations.
- The complete-host consumer-flow fixture runs `SvgMapping` over the whole host, grafts the processed complete document into the original host exactly once, and only then selects embedded SVG roots.
- The two-SVG phase of that consumer-flow golden uses Standoc's evolving collision map and repeated namespaced root IDs: each SVG gets a freshly constructed `D -> S -> N` pipeline from the current immutable snapshot, the first root remains unchanged, the rewritten IDs are merged between constructions, and the later root receives its positional `_inject_1` suffix.
- Mutating the caller-owned collision map after a pipeline is constructed does not change that pipeline's deeply frozen snapshot, and executing a pipeline does not mutate caller state.
- A separate fixture/example pins Vectory's batch `SvgIdDisambiguator.duplicate_ids` behavior without using it as the Standoc migration contract.
- The consumer-flow golden proves the whole-document graft retains host-root metadata and the later per-SVG namespace steps retain requested final declarations.
- Each reparsed output retains the references, classes, namespaces, and host metadata relevant to its included operations.
- Reapplying each of the three executable pipeline profiles produces XML-equivalent output.
- Reapplying the complete `SvgMapping` result graft and per-SVG consumer orchestration produces an XML-equivalent host document.
- README and API reference examples call the tested public methods, document only the three executable profiles, and reproduce Card 02's accepted namespace contract without adding policies.
- The documentation calls `SvgClassSuffixer` the canonical Vectory implementation, distinguishes #1298's ID-only adoption from the separately tracked class-suffix adoption task, and keeps SVG-to-PNG projection separate.
- `90-progress.md` records the URL of the separately tracked **Adopt `Vectory::SvgClassSuffixer` in Standoc** task before the #77 handoff closes.
- The production Jekyll documentation build succeeds.
- All focused phase-2 specs and the pre-existing disambiguator/mapping/document specs pass.
- Style and whitespace checks pass for every changed Ruby and documentation file.

Planned checks (not yet run):

```sh
bundle exec rspec spec/vectory/svg_class_suffixer_spec.rb spec/vectory/svg_namespace_binding_contract_spec.rb spec/vectory/svg_namespace_binding_spec.rb spec/vectory/svg_subtree_graft_spec.rb spec/vectory/svg_id_disambiguator_spec.rb spec/vectory/svg_transform_pipeline_spec.rb spec/vectory/svg_transform_integration_spec.rb spec/vectory/svg_mapping_spec.rb spec/vectory/svg_document_spec.rb
bundle exec rubocop vectory.gemspec lib/vectory.rb lib/vectory/svg_class_suffixer.rb lib/vectory/svg_namespace_binding.rb lib/vectory/svg_subtree_graft.rb lib/vectory/svg_transform.rb lib/vectory/svg_transform_pipeline.rb spec/vectory/svg_class_suffixer_spec.rb spec/vectory/svg_namespace_binding_contract_spec.rb spec/vectory/svg_namespace_binding_spec.rb spec/vectory/svg_subtree_graft_spec.rb spec/vectory/svg_transform_pipeline_spec.rb spec/vectory/svg_transform_integration_spec.rb
cd docs && JEKYLL_ENV=production bundle exec jekyll build --trace
git diff --check
```

## Status
State: implemented; publication, required lint and CI pending; downstream adoption-task URL pending
PR: not yet published (GitHub access unavailable)
