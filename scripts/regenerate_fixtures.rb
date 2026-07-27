#!/usr/bin/env ruby
# frozen_string_literal: true

# One-shot fixture regeneration script.
# Run from the vectory root: `bundle exec ruby scripts/regenerate_fixtures.rb`

require_relative "../lib/vectory"

Pairs = [
  ["spec/examples/emf2eps/img.emf", :to_eps, "spec/examples/emf2eps/ref.eps"],
  ["spec/examples/emf2ps/img.emf",  :to_ps,  "spec/examples/emf2ps/ref.ps"],
  ["spec/examples/eps2emf/img.eps", :to_emf, "spec/examples/eps2emf/ref.emf"],
  ["spec/examples/eps2ps/img.eps",  :to_ps,  "spec/examples/eps2ps/ref.ps"],
  ["spec/examples/eps2svg/img.eps", :to_svg, "spec/examples/eps2svg/ref.svg"],
  ["spec/examples/ps2emf/img.ps",   :to_emf, "spec/examples/ps2emf/ref.emf"],
  ["spec/examples/ps2eps/img.ps",   :to_eps, "spec/examples/ps2eps/ref.eps"],
  ["spec/examples/ps2svg/img.ps",   :to_svg, "spec/examples/ps2svg/ref.svg"],
  ["spec/examples/svg2emf/img.svg", :to_emf, "spec/examples/svg2emf/ref.emf"],
  ["spec/examples/svg2eps/img.svg", :to_eps, "spec/examples/svg2eps/ref.eps"],
  ["spec/examples/svg2ps/img.svg",  :to_ps,  "spec/examples/svg2ps/ref.ps"],
].freeze

Pairs.each do |input, method, output|
  ext = File.extname(input).delete(".")
  klass_name = { "emf" => "Emf", "eps" => "Eps", "ps" => "Ps",
                 "svg" => "Svg" }.fetch(ext)
  klass = Vectory.const_get(klass_name)
  result = klass.from_path(input).public_send(method)
  File.binwrite(output, result.content)
  puts "[OK] #{input} -> #{output} (#{result.content.bytesize} bytes)"
rescue => e
  puts "[FAIL] #{input} -> #{output}: #{e.class}: #{e.message}"
end
