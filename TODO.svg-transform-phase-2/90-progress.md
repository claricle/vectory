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

Required changed-file RuboCop is blocked before inspection: its inherited remote
configuration is absent from the cache and cannot be fetched in this environment.
A supplemental default-config diagnostic was not clean and does not replace
repository lint. The full suite and GitHub Actions have not been reported green.

## Remaining acceptance and publication

1. Restore GitHub access, push this implementation branch, and open a PR
   referencing #77 without automatically closing it. Keep the remote updated.
2. Run changed-file RuboCop with the actual inherited configuration and resolve
   applicable findings. Obtain required CI and approvals before merge.
3. Create or identify **Adopt `Vectory::SvgClassSuffixer` in Standoc** and record
   its actual URL here. GitHub access prevented this handoff; no URL is fabricated.
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
