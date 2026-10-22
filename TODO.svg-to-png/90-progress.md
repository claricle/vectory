# Progress and handoff

## Current state

Planning is complete; implementation has not started. All six work cards remain `todo`. Placement, patched engine base, owned PNG encoder, binding, seven-target native matrix, unsupported-platform shell package, font resolver, self-contained-SVG policy, CLI surface, and conformance integration are fixed in the plan. The public API has no resource-injection surface: same-document fragments and inline `data:` values are allowed, raw DTDs and every non-inline external reference are rejected before entity expansion or rendering, and no secondary filesystem or network access is permitted. No implementation card is waiting on an owner decision.

## Next executable action

As soon as the new `claricle/svgpng` repository is available, execute Card 01 there: scaffold the public values and record the complete native/font/external-reference/PNG contract, including the fixed intrinsic-size, profile precedence, rounding, aspect-fit, and background algorithm. Then implement Card 02's owned shim, encoder, and resvg fork. Card 03's Ruby font/profile work may begin after Card 01, but its font-enforcement and parser-side rejection checks depend on Card 02's ABI and fork.

## Blockers

- The `claricle/svgpng` repository is being created and must exist before Cards 01–03 and 05 can be committed.
- Card 04 cannot start until Cards 01–03 finalize the public API/error contract and produce a cleanly installable, unpublished `svgpng` 0.1.0 native gem for Vectory's development/CI platform, with recorded artifact checksum and smoke evidence. It cannot merge or release until Card 05 publishes the exact tested seven native packages and `ruby`-platform shell package to Vectory's configured gem source.
- The seven-platform native and golden jobs require suitable CI runners, signing/release credentials where applicable, and a working cross-build strategy; these are delivery prerequisites, not open architecture decisions.

## Remaining evidence gaps

- Create the Claricle resvg fork, record the exact fork commit and patch digest, and verify the callback can select the loaded face used by shaping without expanding beyond the documented narrow delta.
- Measure installed gem/shared-library size and dynamic dependencies for each platform artifact; upstream CLI size is only a directional comparison.
- Confirm each CI runner or cross-toolchain can build and smoke-test the pinned resvg/libpng/zlib closure without changing the fixed output profile.
- Validate parser-side rejection against resvg's complete set of SVG and CSS external-reference sites; any path that reaches a filesystem or network loader is a release blocker, and the filesystem resources directory must remain unset.
- Compare the final #77 branch before Card 06 lands and reconcile `vectory.gemspec`, `lib/vectory.rb`, `README.adoc`, and `docs/reference/api.adoc`.

## Remaining sequence

1. Complete Card 01 in `claricle/svgpng`.
2. Implement Card 02; start Card 03's font/profile work in parallel after Card 01, then finish its native enforcement and external-reference checks against Card 02.
3. Build the unpublished eight-package 0.1.0 release-candidate set, record its checksums and clean supported/unsupported resolution evidence, and start Card 04 against the local development-platform native gem.
4. Execute Card 05's seven-platform byte gate, publish those exact seven native gems plus the `ruby` shell only after it passes, then merge Card 04 and permit Vectory to release or advertise PNG output.
5. Complete Card 06 after Card 05 passes and after reconciling #77's shared files.
