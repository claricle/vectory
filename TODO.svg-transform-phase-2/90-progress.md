# Progress and handoff

## Current state

- Planning is complete against `origin/main` at `cbe96286a2de59508eba616c421fcd44c2d95070`.
- The Crass dependency, canonical Vectory class-suffix contract, Standoc ownership boundary, and issue #76 landing order are settled and incorporated into Cards 01–05.
- A local Crass 1.0.7 probe confirms that `.foo\+bar` exposes semantic value `foo+bar`, preserves raw spelling `foo\+bar`, and serializes as `.foo\+bar_inject_0` when both forms receive the suffix. Card 01 pins this behavior instead of leaving escaped identifiers as a contingency.
- The transform execution boundary is fixed: the only executable profiles are whole-document `G`, per-SVG `D -> S -> N`, and selected-SVG `G -> D -> S -> N`. For the Standoc boundary, run `SvgMapping` and `G` over the complete host once, then construct a fresh `D -> S -> N` pipeline for each selected SVG from the current immutable collision-map snapshot and positional index. Merge post-rewrite IDs before constructing the next pipeline; the first repeated namespaced root ID remains unchanged and the later root receives `_inject_1`. Vectory's batch `duplicate_ids` path remains a separate contract.
- The namespace capability is required, but its detailed public contract is not yet evidenced. Card 02 now starts with representative consumer fixtures and parser probes and records the selected source scope, alias, shadow, collision, prefix-to-default, unbind, and error policies before implementation.
- No implementation has started; every card remains `todo` with no PR.

## Next executable action

Start Cards 01 and 03 behavioral files, fixtures, and focused specs, and start only Card 02's namespace fixtures, parser probe, and `contract_cases.yml` decision matrix. Do not implement namespace mutation until that matrix is accepted. Do not edit the shared `lib/vectory.rb` autoload table in those slices; Card 04 owns that integration after all three primitives are ready. Card 01 adds the fixed `crass ~> 1.0, >= 1.0.7` runtime dependency and establishes the canonical class-suffix contract in Vectory.

## Blockers and conditions

- Card 02 implementation waits for its evidence matrix to select the namespace contract. If the representative fixtures and XML rules cannot distinguish a consumer-visible policy, obtain an authoritative namespace input/expected-output example.
- Card 04 waits for Cards 01–03 and Card 02's accepted contract because all four primitives participate in its bounded profile contract.
- Card 05 waits for Card 04's scope rules and exact three-profile set.
- Issue #76's Vectory integration waits for issue #77 to land, then rebases and reconciles the four shared public files.
- Standoc #1298 adopts only ID disambiguation and retains its inline Crass helpers. Creating or identifying and recording the URL of the separate **Adopt `Vectory::SvgClassSuffixer` in Standoc** task is a handoff condition for Card 05, not a Vectory implementation blocker.

## Remaining gaps to close during execution

- Implement Card 01's pinned escaped-identifier contract by matching Crass's semantic value, preserving and suffixing its raw spelling, and comparing first-pass/repeat output to the checked-in golden.
- Run Card 02's representative default, prefixed-attribute, unbind, redeclaration, shadow, target-collision, and prefix-to-default probes. Record which source-resolution, alias-retention, shadow, collision, attribute-prefix, and error behaviors the consumer fixtures actually require; then implement and pin only that accepted contract.
- Verify deep-copy/reparent behavior for comments, CDATA, and processing instructions while implementing Card 03, and preserve the documented `Vectory::ParsingError` class and message contracts.
- Keep Card 04's executable set equal to `G`, `D -> S -> N`, and `G -> D -> S -> N`. Permit only construction prefixes of those profiles, reject calling incomplete prefixes, and reject every skipped, duplicate, reversed, or non-leading graft sequence.
- Make the disambiguation step defensively copy and deeply freeze its collision-map snapshot. In the Standoc flow, build a fresh per-SVG pipeline after each caller-owned map merge; never reuse one pipeline across evolving map states.
- Keep Card 05's three-entry manifest, three pipeline goldens, primitive goldens, and complete-host consumer-flow golden in lockstep. In that flow, map the complete host, graft it once, then build fresh per-SVG pipelines in document order: the first repeated root ID stays unchanged, the later root gets `_inject_1`, and namespace declarations follow Card 02's accepted contract. Pin batch `duplicate_ids` separately.
- Preserve #1298's `SvgIdDisambiguator` handoff without claiming that it adopts `SvgClassSuffixer`; #77 establishes the canonical Vectory implementation, while the separately tracked **Adopt `Vectory::SvgClassSuffixer` in Standoc** task owns downstream replacement and removal.
- Hand issue #76 the final `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc` shapes after issue #77 lands.

## Handoff order

1. Implement Cards 01 and 03 while Card 02 resolves its contract matrix, then implement Card 02 against the accepted cases; avoid concurrent edits to `lib/vectory.rb`.
2. Let Card 04 integrate the primitive and pipeline autoloads in `lib/vectory.rb`, then add the host-capturing graft adapter, immutable collision-map snapshots, exact scope checks, and the three-profile table.
3. Finish the whole-document graft plus two-SVG fresh-snapshot fixture, Standoc migration contract, documentation, downstream class-suffix adoption-task URL, and focused verification in Card 05.
4. Land issue #77 before issue #76's small Vectory integration card reconciles the shared files.
