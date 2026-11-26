#!/usr/bin/env ruby
# frozen_string_literal: true

# Generates the reverse lane's Ruby ground truth: for every golden SVG the
# forward gate pins (the emfsvg checkout's spec/fixtures/golden), runs the
# Ruby SVG -> EMF translation (Emfsvg.from_svg) and writes the resulting EMF
# bytes as <name>.emf into the output directory. The emf90-parity harness
# compares the Rust round-trip output against these bytes file-by-file.
#
#   EMF_GOLDEN_ROOT=/path/to/emfsvg/spec/fixtures/golden \
#     bundle exec ruby vectory-rs/scripts/gen_reverse_golden.rb [OUT_DIR]
#
# Defaults: EMF_GOLDEN_ROOT env or /tmp/emfsvg-parity-corpus goldens are NOT
# used here (they carry EMF sources, not SVG goldens); OUT_DIR defaults to
# vectory-rs/testdata/reverse-golden. Files whose translation raises get no
# output and are reported — the harness treats a missing golden as an
# engine-error class, not a parity failure.

require "fileutils"

gem_root = File.expand_path("../../..", __dir__)
$LOAD_PATH.unshift(File.join(gem_root, "lib"))
require "emfsvg"

golden_root = ENV["EMF_GOLDEN_ROOT"] ||
              File.expand_path("../../emfsvg/spec/fixtures/golden", gem_root)
out_dir = ARGV[0] || File.join(gem_root, "vectory-rs", "testdata", "reverse-golden")

sources = Dir.glob(File.join(golden_root, "*.svg")).sort
abort "no golden SVGs under #{golden_root}" if sources.empty?

FileUtils.mkdir_p(out_dir)
ok = 0
errors = []
sources.each do |file|
  name = File.basename(file, ".svg")
  begin
    emf = Emfsvg.from_svg(File.read(file))
    File.binwrite(File.join(out_dir, "#{name}.emf"), emf)
    ok += 1
  rescue StandardError => e
    errors << "#{name}: #{e.class}: #{e.message[0, 100]}"
  end
end

puts "goldens written: #{ok} -> #{out_dir}"
errors.each { |e| puts "error: #{e}" }
