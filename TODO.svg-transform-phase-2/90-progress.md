# Progress and handoff

## Implemented

The implementation is on `feat/svg-transform-pipelines`, based on plan merge
`cf87de853415651a39efbffc2c99d522813381e3`. All five cards have implementation,
focused specs and documented contracts. Publication and acceptance checks below
remain open; issue #77 is not yet complete.

- Class suffixing uses Crass and preserves escaped selector spelling, comments,
  strings and nested rules. Repeating the same suffix is stable.
- Namespace binding preserves element and attribute expanded names. The contract
  matrix covers aliases, shadowing, conflicting remaps, used unbinds, default
  namespace capture and namespaced attributes. Unsafe operations fail.
- Serialized grafting retains the captured host root and XML envelope, including
  comments, processing instructions and CDATA.
- Immutable transforms compose only `G`, `D -> S -> N`, and selected-SVG
  `G -> D -> S -> N`. Invalid order, incomplete profiles and incompatible scopes
  fail. Captured options are deeply frozen.
- Goldens cover all profiles, repeated execution, real whole-host mapping and
  grafting, fresh sequential collision-map snapshots, repeated root IDs,
  nested SVG scope, and separate batch-disambiguation behavior.
- README and API docs describe the migration boundary. Jekyll dependencies that
  recent Ruby versions no longer bundle are explicit in `docs/Gemfile`.

## Verification

On Ruby 3.4.8, the combined focused suite passed **189 examples, 0 failures,
0 pending**: class suffixer, namespace matrix and behavior, graft, ID
disambiguator, pipeline, integration, mapping and document specs.
The production Jekyll build and executable API migration example passed.
Final implementation review found no demonstrated functional defect.

Changed-file RuboCop with the actual inherited configuration passes for all 13
Ruby files. After the lint changes, the seven affected specs pass **168 examples,
0 failures**. Assertion-heavy examples were split without removing assertions.

Draft PR: https://github.com/claricle/vectory/pull/80. At head `976a69f`, all nine
Ruby/platform test jobs and the documentation build passed. The link checker
failed before checking links because `docs/lychee.toml` repeated the `exclude`
key; those entries are now consolidated and the configuration parses. The final
test gate failed on repository dispatch with "Resource not accessible by
integration"; this requires CI credential/configuration resolution. New-head
CI remains required before merge. The full suite has not been reported green
for the final head.

## Remaining acceptance and publication

1. Keep draft PR #80 updated with implementation and verification evidence.
2. Obtain required final-head CI and approvals before merge, including resolving
   the repository-dispatch credential/configuration failure.
3. Standoc adoption is tracked at
   https://github.com/metanorma/metanorma-standoc/issues/1301 and depends on a
   Vectory release containing #77.
4. After #77 lands, #76's Vectory adapter must rebase and reconcile
   `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and
   `docs/reference/api.adoc`. Keep rendering distinct from content transforms.

## Standoc boundary

Standoc #1298 adopts ID disambiguation and retains its inline Crass helpers.
The separate adoption task must depend on a release containing #77 and replace
those helpers only after compatibility specs pass. Map and graft the full host
once, then construct a fresh D/S/N pipeline per outer SVG from the current
collision-map snapshot. Merge rewritten IDs before the next pipeline: the first
repeated root stays unchanged and the later root gets `_inject_1`.
Batch `duplicate_ids` remains a separate contract.
