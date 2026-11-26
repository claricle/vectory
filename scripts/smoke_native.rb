#!/usr/bin/env ruby
# frozen_string_literal: true

# The per-platform native smoke (lane 3, claricle/vectory#99): manipulate
# one corpus SVG through the native engine and compare with the Ruby lane.
# Works against a checkout (via bundler) or an installed gem (plain
# require) so the cross-gem matrix can reuse it verbatim.
#
# Usage: smoke_native.rb <dylib> <corpus-svg> [<fixture-input>]
#
# Exit 0 when both cohorts answer identically; nonzero otherwise.

dylib = ARGV.fetch(0) { abort "usage: smoke_native.rb <dylib> <corpus-svg> [<fixture-input>]" }
corpus = ARGV.fetch(1) { abort "usage: smoke_native.rb <dylib> <corpus-svg> [<fixture-input>]" }
fixture = ARGV[2]

ENV["VECTORY_NATIVE_LIB"] = dylib
require "vectory"
require "leptris"

content = File.read(corpus)
ruby_bytes = Leptris::XML.parse(content).root.to_xml.b
native_bytes = Vectory::Runtime.roundtrip(content, profile: :ruby_xml).to_s.b
roundtrip_same = ruby_bytes == native_bytes

puts "smoke roundtrip #{File.basename(corpus)}: " \
     "#{roundtrip_same ? 'same' : 'DIFFERENT'} (bytes)"

if roundtrip_same == false
  warn "native roundtrip mismatch"
  exit 1
end

if fixture
  require "canon"
  require "canon/rspec_matchers"
  steps = [
    { "name" => "disambiguate", "index" => 1, "duplicate_ids" => { "drawing" => 2, "paint" => 2 } },
    { "name" => "suffix_classes", "index" => 1 },
    { "name" => "bind_namespaces",
      "namespaces" => { "bind" => { "link" => "http://www.w3.org/1999/xlink" },
                        "unbind" => %w[xlink unused],
                        "remap" => { "http://www.w3.org/1999/xlink" => "link" } } },
  ]
  pipeline_input = File.read(fixture)

  ENV["VECTORY_RUNTIME"] = "ruby"
  ruby_output = Vectory::Runtime.pipeline(pipeline_input, steps: steps)
  ENV["VECTORY_RUNTIME"] = "native"
  native_output = Vectory::Runtime.pipeline(pipeline_input, steps: steps)

  matcher = Canon::RSpecMatchers::SerializationMatcher.new(ruby_output, :xml,
                                                           match_profile: :spec_friendly)
  pipeline_same = matcher.matches?(native_output)
  puts "smoke svg_cleanup #{File.basename(fixture)}: " \
       "#{pipeline_same ? 'same' : 'DIFFERENT'} (canon)"
  unless pipeline_same
    warn "native pipeline mismatch"
    warn matcher.failure_message
    exit 1
  end
end

puts "native vectory smoke: OK"
