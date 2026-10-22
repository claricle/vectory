# 01 — Suffix SVG classes with Crass

Effort: **12–18 engineer-hours**.

## Purpose

Establish a parser-backed Vectory class-attribute and stylesheet-selector suffixing primitive from Standoc's inline behavior without carrying forward its top-level-only at-rule limitation. Standoc adoption and removal of its local implementation remain a required downstream task.

Crass is the approved parser dependency. There is no regex fallback or dependency-rejection branch.

## Changes

- Add `crass` to `vectory.gemspec` with constraints `~> 1.0`, `>= 1.0.7`, and leave the lockfile resolution to the repository's normal Bundler workflow.
- Add `lib/vectory/svg_class_suffixer.rb` with `Vectory::SvgClassSuffixer.rewrite(content, suffix)`. Queue its `lib/vectory.rb` autoload registration for Card 04's integration owner instead of editing the shared table concurrently.
- Establish the Crass-based class-attribute and selector suffixing contract represented by Standoc's `lib/metanorma/cleanup/image.rb` lines 171–185 in this Vectory primitive. Standoc #1298 retains the local implementation; the separate **Adopt `Vectory::SvgClassSuffixer` in Standoc** task owns later adoption and removal after a #77-containing release.
- Parse SVG XML with Leptris and each `<style>` value with Crass. Recursively parse rule-bearing at-rule blocks, then rewrite class-selector delimiter/identifier token pairs, including those nested in selector functions.
- Suffix XML class-list tokens under each SVG root once, preserve token order, and recognize an already-applied suffix so the same operation is idempotent.
- Preserve declarations, `url()` fragments, comments, whitespace, strings, IDs, element selectors, and attribute selectors. Avoid using `selector[:value]` as the mutation source when Crass serialization is driven by tokens/raw values.
- Match escaped selector identifiers by their decoded Crass `:value` against the XML class token while preserving their Crass `:raw` spelling. Append the safe `_inject_<index>` suffix to both forms: `.foo\+bar` matching XML class `foo+bar` must serialize as `.foo\+bar_inject_0` while the XML token becomes `foo+bar_inject_0`. Detect an existing suffix from the semantic value so repeat application changes neither representation.
- Follow Crass's tolerant parsing model for malformed CSS: preserve unrecognized or incomplete token streams and rewrite only selector tokens that can be identified safely; invalid SVG XML remains an error.
- Add `spec/vectory/svg_class_suffixer_spec.rb` with focused examples for multiple SVGs, compound selectors, pseudo-classes/functions, comma lists, nested `@media`/`@supports`, comments, escaped identifiers, non-selector dots, empty style/class values, malformed CSS behavior, and repeat application.
- Add small input/output fixtures under `spec/fixtures/svg_transform/` based on existing repository SVG style content and the Standoc `_inject_<index>` convention. Include an escaped-identifier input with XML class `foo+bar` and selector `.foo\+bar`, its first-pass `_inject_0` output, and an XML-equivalent repeat output.

## Dependencies

- The fixed runtime dependency is `crass ~> 1.0, >= 1.0.7`.
- Preserve the current `SvgIdDisambiguator` string-in/string-out and Leptris parsing conventions.
- Use Standoc's current class suffixing as a read-only compatibility oracle; this card does not edit the consumer.
- Coordinate the `lib/vectory.rb` registration through Card 04's integration owner.

## Done when

- Every tested XML class token receives the requested suffix once.
- Every tested class selector receives the requested suffix at every tested at-rule depth.
- An escaped selector `.foo\+bar` continues matching XML class token `foo+bar` after both become suffixed, retains its escape in serialized CSS, and is unchanged by repeat application.
- Declaration content and unrelated selectors remain byte-for-byte unchanged in the focused fixtures.
- Reapplying the same suffix produces XML-equivalent output with no doubled suffixes.
- The gem cannot resolve to a Crass release older than 1.0.7.
- The focused class-suffixer spec passes against the golden fixtures.

Planned checks (not yet run):

```sh
bundle exec rspec spec/vectory/svg_class_suffixer_spec.rb
bundle exec ruby -e 'spec = Gem::Specification.load("vectory.gemspec"); dep = spec.runtime_dependencies.find { |item| item.name == "crass" }; abort "missing secure crass constraint" unless dep && dep.requirement.satisfied_by?(Gem::Version.new("1.0.7")) && !dep.requirement.satisfied_by?(Gem::Version.new("1.0.6"))'
```

## Status
State: todo
PR: none
