# frozen_string_literal: true

# Regenerates vectory-rs/testdata/golden — the Ruby producer's bytes for the
# vectory corpus (spec/**/*.svg), serialized exactly as the vectory writer
# does: Leptris::XML.parse(content).root.to_xml.
#
#   ruby vectory-rs/scripts/gen_ruby_golden.rb [REPO_ROOT] [OUT_DIR]
#
# Defaults: REPO_ROOT = the repository root above this script;
# OUT_DIR = REPO_ROOT/vectory-rs/testdata/golden.
#
# Files that leptris parses but that have no root element (vectory itself
# raises ParsingError on them) are reported and get no golden — the Rust
# round-trip test asserts they fail decode with a named error.

require "leptris"
require "fileutils"

script_dir = __dir__
default_root = File.expand_path("../..", script_dir)
repo_root = ARGV[0] || default_root
out_dir = ARGV[1] || File.join(repo_root, "vectory-rs", "testdata", "golden")
spec_root = File.join(repo_root, "spec")

ok = 0
fails = []
Dir.glob("#{spec_root}/**/*.svg").sort.each do |file|
  rel = file.delete_prefix("#{spec_root}/")
  dest = File.join(out_dir, rel)
  FileUtils.mkdir_p(File.dirname(dest))
  begin
    content = File.read(file)
    document = Leptris::XML.parse(content)
    raise "no root element" unless document.root

    File.binwrite(dest, document.root.to_xml)
    ok += 1
  rescue StandardError => e
    fails << "#{rel}: #{e.class}: #{e.message[0, 80]}"
  end
end

puts "goldens written: #{ok}"
puts fails
