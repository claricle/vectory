# 03 — Graft processed subtree content

Effort: **8–12 engineer-hours**.

## Purpose

Make Standoc's post-`SvgMapping` reparse-and-splice behavior a tested Vectory primitive while keeping Vectory independent of the caller's Nokogiri/Moxml object model.

## Changes

- Add `lib/vectory/svg_subtree_graft.rb` with `Vectory::SvgSubtreeGraft.replace_root_content(host_content, processed_content)`. Queue its `lib/vectory.rb` autoload registration for Card 04's integration owner instead of editing the shared table concurrently.
- Parse both serialized inputs with Leptris, retain the host document and root node, remove the host root's content, and append deep copies of the processed root's complete content sequence.
- Preserve the host root name, attributes, namespace declarations, and document-level content. Transfer element, text, comment, CDATA, and processing-instruction children in source order.
- Raise `Vectory::ParsingError` for every input-structure failure. Wrap malformed host XML with a message matching `\Ainvalid host_content XML:`, wrap malformed processed XML with `\Ainvalid processed_content XML:`, and use the exact messages `host_content has no document root` and `processed_content has no document root` for well-formed documents without document elements. Ensure nodes are copied/reparented safely rather than leaving them tied to a discarded document.
- Make repeated grafting with the same processed content XML-equivalent.
- Add `spec/vectory/svg_subtree_graft_spec.rb` and checked-in host/processed/expected-output golden sets under `spec/fixtures/svg_transform/subtree_graft/` for cases matching `svgmap_cleanup`: changed processed children, retained host-root metadata, mixed content, namespaces, empty content, and repeat application. Add separate malformed-host, malformed-processed, rootless-host, and rootless-processed examples that assert the exact class and message contract above.

## Dependencies

- Its behavioral file, fixtures, and focused spec are independent of Cards 01 and 02; coordinate the shared `lib/vectory.rb` registration through Card 04's integration owner.
- The observable compatibility target is Standoc's current `svgmap_cleanup`: replace root children, retain root attributes, return a complete host document.

## Done when

- Grafting replaces only the host root's content.
- The host root name, attributes, and namespace declarations remain unchanged.
- Document-level content remains unchanged.
- Every supported child node kind retains its source order after grafting.
- Grafted names retain their namespace identities after reparsing.
- Malformed host and processed XML raise `Vectory::ParsingError` with their documented stable message prefixes and expose no partial result.
- Well-formed host and processed documents without roots raise `Vectory::ParsingError` with their documented exact messages and expose no partial result.
- Repeating the graft produces XML-equivalent output.
- Every successful focused graft case matches a checked-in expected XML golden.
- The focused graft spec passes.

Planned checks (not yet run):

```sh
bundle exec rspec spec/vectory/svg_subtree_graft_spec.rb
```

## Status
State: todo
PR: none
