# Issue #77 execution plan — SVG transform layer phase 2

## Problem

Vectory has a string-in/string-out `Vectory::SvgIdDisambiguator`, but the rest of the SVG mutation sequence remains split between Vectory and Metanorma Standoc. Standoc still owns class-attribute suffixing, CSS selector rewriting, and the reparse/splice needed after `SvgMapping`. The missing operations cannot yet be expressed as named, validated, idempotent transforms.

The issue is [claricle/vectory#77](https://github.com/claricle/vectory/issues/77). Standoc's ID-disambiguation adoption is separately tracked by [metanorma/metanorma-standoc#1298](https://github.com/metanorma/metanorma-standoc/issues/1298); class-suffix adoption belongs to the required downstream task named below.

## Scope

- Add a Crass-backed SVG class suffixer that rewrites class selector tokens in `<style>` content and matching XML `class` tokens.
- Add a namespace declaration transform for binding, unbinding, and remapping namespaces on an SVG subtree.
- Add a serialized subtree graft primitive that replaces host-root content while preserving the host root and its attributes.
- Wrap subtree grafting, `SvgIdDisambiguator`, class suffixing, and namespace binding in named transform values that compose with `>>`, validate their order and execution scope, and expose their name/options for inspection.
- Pin unit and end-to-end behavior with real SVG/CSS golden fixtures, idempotence examples, public API documentation, and focused specs.

## Exclusions

- Do not change Metanorma Standoc in this issue. Standoc #1298 adopts only ID disambiguation and deliberately retains its local CSS helpers; a required downstream task, **Adopt `Vectory::SvgClassSuffixer` in Standoc**, must remove those helpers after #77 is released.
- Do not add a regular-expression fallback for CSS selector rewriting.
- Do not implement general CSS selector semantics. The class suffixer rewrites Crass class-selector token patterns and leaves declarations, strings, URLs, and unrelated selectors unchanged.
- Do not broaden `SvgDocument#namespace` or silently change existing `SvgMapping` behavior; the new namespace API is separate and explicitly named.
- Do not add Nokogiri as a runtime dependency or accept caller-specific DOM objects in the public transform contract.
- Do not include SVG-to-PNG rendering from [claricle/vectory#76](https://github.com/claricle/vectory/issues/76).

## Intended end state

- `Vectory::SvgClassSuffixer.rewrite(content, suffix)` is the canonical Vectory class/CSS suffixing implementation available for downstream adoption. Standoc keeps its inline implementation until the separate adoption task lands.
- `Vectory::SvgNamespaceBinding` applies the namespace bind, unbind, and remap contract selected by Card 02's measured consumer-fixture probe; the public signature and edge-case policy are documented only after that record is accepted.
- `Vectory::SvgSubtreeGraft.replace_root_content(host_content, processed_content)` retains the original host root and replaces only its content.
- `Vectory::SvgTransform` factories return named, inspectable graft, disambiguation, class-suffix, and namespace steps. The graft factory captures the host serialization, making the otherwise binary operation unary for composition; `Vectory::SvgTransformPipeline` composes the steps with `>>`, rejects invalid ordering or scope, and exposes only the three evidenced executable profiles: whole-document `G`, per-SVG `D -> S -> N`, and selected-SVG `G -> D -> S -> N`.
- The existing `SvgIdDisambiguator` remains source compatible and is adapted rather than duplicated.
- A Standoc-style host is orchestrated at two explicit scopes: run `SvgMapping` over the complete host, graft that processed document back into the original complete host once, then select embedded SVG roots in document order. For each SVG, construct a fresh `D -> S -> N` pipeline from an immutable snapshot of Standoc's current caller-owned collision map and that SVG's positional index; after replacement, the caller merges the post-rewrite IDs before constructing the next pipeline. Do not replace this compatibility path with `SvgIdDisambiguator.duplicate_ids`, because its batch result would suffix both repeated root IDs instead of leaving the first unchanged.
- The complete named order is `graft_into -> disambiguate -> suffix_classes -> bind_namespaces` (`G -> D -> S -> N`). `G` alone accepts a complete host document; the complete four-step profile requires the captured host root to be `svg`; the per-SVG profile accepts one selected SVG root. Other subsequences are not public executable pipelines.
- Repeating each primitive and each of the three supported pipeline profiles with the same immutable inputs produces an equivalent result.

## Acceptance mapping

- Issue item 1 is delivered and primitive-golden-pinned by Card 01's `suffix_classes` implementation, then exercised inside the supported `D -> S -> N` and `G -> D -> S -> N` pipeline goldens in Card 05.
- Issue item 2 is delivered by Card 02's measured namespace-contract record, implementation, and primitive golden fixtures; Card 04 then adapts the selected contract as `N`.
- Issue item 3 is delivered by Card 03's root-content graft primitive and Card 04's host-capturing `G` profile; Card 05 independently golden-pins that profile.
- Issue item 4 is delivered by Card 04's order- and scope-checked `G`, `D -> S -> N`, and `G -> D -> S -> N` profiles; Card 05 idempotence-tests and golden-pins those three evidenced shapes while Cards 01–03 pin the public primitives individually.

## Ordering

1. Complete Card 01 and Card 03 independently. Begin Card 02 with its namespace contract probe and record; implement that primitive only after the record selects the supported signature and edge-case policies. The primitive branches must not edit the shared `lib/vectory.rb` autoload table concurrently; Card 04 owns that integration after the three slices are ready. Card 01 includes the approved Crass dependency and the canonical Vectory class-suffix contract.
2. Complete Card 04 after all four primitive operations, including Card 03's graft, exist.
3. Complete Card 05 after Card 04 stabilizes the full named pipeline contract.
4. Land issue #77 before issue #76's Vectory integration card reconciles the four shared public files.

Cards 01–03 have independent behavior, files, fixtures, and focused specs after Card 02's contract gate, but share one integration boundary in `lib/vectory.rb`; Card 04 owns those autoload additions after the three primitive slices are ready. Card 04 also makes the graft composable by capturing its host input in an immutable transform value, while preserving the separate whole-document and selected-SVG execution scopes. Card 05 pins the three supported profiles and the Standoc-style whole-document-then-per-SVG orchestration.

## Effort

| Card | Estimate |
| --- | ---: |
| 01 — Suffix SVG classes with Crass | 12–18 engineer-hours |
| 02 — Bind SVG subtree namespaces | 16–26 engineer-hours |
| 03 — Graft processed subtree content | 8–12 engineer-hours |
| 04 — Compose named transform pipelines | 12–18 engineer-hours |
| 05 — Pin golden contracts and documentation | 14–20 engineer-hours |
| **Base total** | **62–94 engineer-hours (7.75–11.75 engineer-days)** |

Hold up to 12 additional hours of contingency, outside the base total, only if the accepted namespace contract requires Leptris behavior beyond the representative probe cases or cross-document node copying exposes behavior not covered by the measured probes. Escaped CSS identifiers are part of Card 01's base contract and fixtures.

The base includes escaped class-selector fixtures; a namespace contract probe plus fixtures for the selected behavior; validation and one golden for each of the three supported executable pipeline profiles; a complete-host `SvgMapping`/graft plus two-SVG sequential-collision-map golden; and the documentation-site build. The remaining technical unknowns stay isolated in the contingency.

## Technical decisions

### Use Crass as a Vectory runtime dependency

- Add Crass with constraints `~> 1.0`, `>= 1.0.7`. Version 1.0.7 is MIT-licensed and has no runtime dependencies.
- It parsed and byte-preserved the repository's real SVG style fixture and a nested `@media`/`@supports` sample; nested blocks require explicit recursion and selector changes require token-level edits.
- Crass parses CSS syntax rather than selector semantics, which matches the bounded class-token operation but must be reflected in tests and documentation.
- Versions before 1.0.7 are excluded because 1.0.7 carries parser denial-of-service fixes. See the [1.0.7 release](https://github.com/rgrove/crass/releases/tag/v1.0.7) and [security advisory](https://github.com/rgrove/crass/security/advisories/GHSA-6wmf-3r64-vcwv).
- Standoc already depends on Crass, so its first consumer does not gain a new dependency family. Vectory's other consumers would.

There is no regex fallback or dependency-rejection branch. `SvgClassSuffixer` and the `suffix_classes` transform are required parts of the phase-two surface.

### Establish the canonical Vectory class suffixer

Standoc #1298 consumes `Vectory::SvgIdDisambiguator` but explicitly retains its Crass helpers. Card 01 establishes the reusable implementation and migration contract in Vectory from the behavior currently represented by `lib/metanorma/cleanup/image.rb` lines 171–185; it does not remove the downstream copy. A separate downstream task named **Adopt `Vectory::SvgClassSuffixer` in Standoc** must be created or identified and its URL recorded before handoff. It must depend on a #77-containing Vectory release, replace the inline class/CSS helpers, preserve Standoc's sequential ID-collision behavior, and remove the now-redundant Crass helper code only after compatibility specs pass.

### Preserve escaped CSS identifiers

Crass 1.0.7 exposes an escaped class selector such as `.foo\+bar` as semantic identifier value `foo+bar` while preserving raw spelling `foo\+bar`. `SvgClassSuffixer` matches the semantic value to the XML class token, appends the safe `_inject_<index>` suffix to both the semantic value and the preserved raw token, and serializes `.foo\+bar_inject_0` to match XML class `foo+bar_inject_0`. It must not rebuild the selector from the decoded value or remove the escape. Card 01 pins first-pass and repeat behavior with a golden fixture.

### Measure and record the namespace contract before implementation

Issue #77 requires bind, unbind, and remap behavior but does not establish an option shape, source-binding scope, declaration-retention rule, shadow policy, collision policy, or error contract. The existing `SvgDocument#namespace` rewrites IDs and links and is not evidence for those declaration semantics. Card 02 therefore starts with representative Standoc-style fixtures and tiny Leptris probes, records expanded-name and declaration outcomes, and selects the smallest public contract that satisfies the observed consumer cases and XML namespace rules before implementation begins.

The record must answer whether remap sources are resolved only at the serialized root or per lexical scope; whether old aliases are retained or removed; how same-URI redeclarations and different-URI shadows behave; how target-prefix collisions are handled; and whether an attribute-bearing prefix-to-default request is rejected or preserves/generates an explicit attribute prefix. The only preselected invariant is the XML rule that a default namespace never qualifies an unprefixed attribute. Exact exception classes and messages follow the selected contract rather than being invented by this plan.

### Use explicit whole-document and selected-SVG boundaries

Let `G`, `D`, `S`, and `N` denote `graft_into`, `disambiguate`, `suffix_classes`, and `bind_namespaces`. `graft_into(host_content:)` captures and validates the original host, then treats `#call`'s argument as the processed serialization whose root content is grafted into that host. This gives the two-input primitive a unary transform interface without hiding either input.

The canonical precedence is `G -> D -> S -> N`. The public executable pipeline contract is deliberately limited to three named profiles backed by the issue and confirmed consumer boundary: `G` for the whole-document graft, `D -> S -> N` for one selected SVG, and `G -> D -> S -> N` for an SVG-root host where all four primitives compose. The last profile proves the issue's graft-composition requirement without declaring every arbitrary subsequence public. Steps and intermediate prefixes may exist while building one of these profiles, but `#call` rejects an incomplete or unrecognized shape. Duplicate, reversed, skipped, and non-leading `G` compositions are not executable public profiles.

A no-`G` profile requires one selected serialized SVG as input. `G` alone accepts any rooted host and processed documents. The complete four-step profile is valid only when the captured host document element is `svg`; that is checked before execution. Nested SVG descendants stay within the selected unit and share its index. Cards 04 and 05 idempotence-test and golden-pin these three profiles; Cards 01–03 golden-pin each primitive independently.

The Standoc consumer boundary is deliberately two-stage rather than one four-step call: run `SvgMapping` on the complete host, apply a `G`-only pipeline once to preserve that host root, then serialize each selected SVG. Before each SVG call, construct a fresh `D -> S -> N` pipeline from an immutable defensive copy of the current collision-map snapshot and the positional index. After replacement, merge that SVG's post-rewrite IDs into the caller-owned map, then construct the next pipeline. Pipelines never mutate the captured snapshot or caller state. This matches `svgmap_cleanup` plus Standoc's `svg_uniqueids` compatibility contract, leaves the first repeated namespaced root ID unchanged, gives the later root its positional suffix, and ensures namespace changes happen after the graft that would otherwise replace root content. Vectory's batch `duplicate_ids` API remains separately documented and tested; it is not the Standoc migration oracle.

## Decisions still needed

The repository owner has no remaining product decision in this plan: the Crass dependency, Vectory ownership boundary, Standoc adoption split, and issue #76 landing sequence remain fixed. Card 02 does have one evidence-gated technical decision before namespace implementation: select and record the public bind/unbind/remap signature and the source-scope, alias-retention, shadow, collision, prefix-to-default, and error policies from representative consumer fixtures, XML rules, and Leptris behavior. If those sources do not distinguish a policy that affects consumers, pause Card 02 and request an authoritative namespace example instead of silently promoting a speculative rule to public API.

## Related work coordination

Issue #76 puts the renderer in a separate `svgpng` gem and leaves only a small integration card in Vectory. Issue #77 lands first. The later #76 integration must rebase and reconcile only `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc`; reserve “transform pipeline” for content mutation so raster projection terminology remains distinct.

Standoc #1298 is the confirmed consumer of `SvgIdDisambiguator` and intentionally retains its inline class suffixer. Card 05 documents the Vectory side of that handoff and the canonical class-suffix behavior without claiming that downstream adoption is complete. Before #77 handoff closes, record the URL of the separate **Adopt `Vectory::SvgClassSuffixer` in Standoc** task; that task, not #1298, owns removal of the inline Standoc implementation.
