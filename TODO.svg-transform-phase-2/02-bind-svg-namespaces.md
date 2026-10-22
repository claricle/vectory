# 02 — Bind SVG subtree namespaces

Effort: **16–26 engineer-hours**: 4–6 for the contract probe and record, then 12–20 for the selected implementation and focused fixtures.

## Purpose

Establish and implement an explicit namespace-declaration transform instead of inferring public behavior from the legacy `SvgDocument#namespace` method, whose current behavior is link and ID rewriting rather than declaration management.

## Changes

- Begin with a contract gate. Add representative Standoc-style XML/SVG inputs under `spec/fixtures/svg_transform/namespace_binding/` and a parser-level probe in `spec/vectory/svg_namespace_binding_contract_spec.rb`. Do not start the public implementation until the probe result and decision matrix described below are recorded in `contract_cases.yml`.
- Cover at least: a root default SVG namespace with unprefixed child elements and attributes; a root `xlink` binding used by an attribute; an unused and a used binding requested for unbind; a same-prefix/same-URI descendant redeclaration; a same-prefix/different-URI descendant shadow; a target prefix already bound to the same and to a different URI; an element-only prefix-to-default request; and an attribute-bearing prefix-to-default request.
- For every probe case, record the input declaration scopes, each relevant element and attribute expanded name, the candidate operation, Leptris's parsed and serialized result, the consumer need represented by the fixture, and the selected public outcome. Compare semantics through reparsed expanded names and in-scope declarations; use byte comparisons only for content whose spelling is itself part of the contract.
- Resolve these proposed questions in that matrix before fixing the API: whether sources are resolved at the serialized root or independently in each lexical scope; whether remapping retains or removes the old alias; how same-URI redeclarations and different-URI shadows are treated; whether target-prefix collisions replace, preserve, or reject; whether attribute-bearing prefix-to-default requests reject or retain/generate an explicit attribute prefix; when unbind is safe; and which exception classes and stable message patterns represent invalid requests.
- Apply the XML Namespaces invariant that a default namespace qualifies unprefixed element names but never unprefixed attributes. Do not select a prefix-to-default policy that changes an attribute's expanded name or emits malformed/unbound QNames.
- If the representative consumer inputs and XML rules do not distinguish a proposed policy that affects serialized output or callers, stop this card and obtain an authoritative input/expected-output example. Do not turn the previous plan's root-only lookup, alias retention, shadow behavior, collision rejection, or exact error text into public API merely because it is convenient to implement.
- After the contract record is accepted, add `lib/vectory/svg_namespace_binding.rb` with the selected serialized-string API. Start from the issue-shaped candidate `Vectory::SvgNamespaceBinding.rewrite(content, bind: {}, unbind: [], remap: {})`, but change and document its option shapes if the recorded consumer cases require URI-qualified or scope-qualified remap information. Queue its `lib/vectory.rb` autoload registration for Card 04's integration owner instead of editing the shared table concurrently.
- Implement only the bind, safe-unbind, and remap cases selected in `contract_cases.yml`. Preflight invalid requests defined by the selected contract, preserve relevant expanded names, and return no partially rewritten serialization on failure.
- Preserve non-target declarations, attributes, child order, comments, text, CDATA, and processing instructions. Repeating every supported request with the same options must be XML-equivalent and must not accumulate declarations or rewrite names again.
- Add `spec/vectory/svg_namespace_binding_spec.rb` and checked-in first-pass and repeat expected outputs for every supported contract case. Add expected-error fixtures only for cases the matrix selects as errors, using the selected exception class and stable message or message pattern.
- Carry the accepted signature, supported cases, rejected cases, and error contract into Card 04's `bind_namespaces` adapter and Card 05's API documentation; neither later card may broaden the matrix implicitly.

## Dependencies

- Leptris already exposes namespace definition add/remove and node namespace assignment APIs; no new runtime dependency is expected.
- The contract probe may run alongside Cards 01 and 03. Namespace implementation follows the accepted matrix. Coordinate the shared `lib/vectory.rb` registration through Card 04's integration owner.

## Done when

- `contract_cases.yml` records every proposed namespace-policy question, the representative consumer case and XML rule that turn on it, and one evidence-backed selected outcome; any unresolved entry keeps this card incomplete and is recorded as a blocker in `90-progress.md`.
- The parser-level contract probe records expanded names and declaration scopes for the default, prefixed-attribute, unbind, redeclaration, shadow, target-collision, and prefix-to-default fixtures.
- No public implementation starts while a consumer-visible matrix entry remains unresolved.
- The selected public method signature and option shapes carry enough namespace identity and scope information to distinguish every accepted remap from an invalid or already-applied request.
- Binding, safe unbinding, and remapping match the accepted contract fixtures without changing relevant element or attribute expanded names unexpectedly.
- The attribute-bearing prefix-to-default case preserves the attribute namespace through an explicit prefix or fails according to the recorded contract; it never silently emits an unqualified attribute.
- Same-URI redeclarations, different-URI shadows, target collisions, alias retention or removal, and repeat application each match their recorded first-pass and repeat expectations.
- Invalid requests use the exception classes and stable message contracts selected in the matrix and return no partially rewritten XML.
- Every successful focused namespace case reparses and matches a checked-in expected XML golden.
- Reapplying every supported namespace operation produces XML-equivalent output without accumulating declarations.
- The focused namespace contract and implementation specs pass.

Planned checks (not yet run):

```sh
bundle exec rspec spec/vectory/svg_namespace_binding_contract_spec.rb spec/vectory/svg_namespace_binding_spec.rb
```

## Status
State: implemented; publication, required lint and CI pending
PR: not yet published (GitHub access unavailable)
