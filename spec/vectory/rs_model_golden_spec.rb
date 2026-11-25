# frozen_string_literal: true

# Guards the vectory-rs golden corpus (vectory-rs/testdata/golden) against
# drift from the Ruby producer: every golden must still equal what the
# vectory writer produces today — Leptris::XML.parse(content).root.to_xml.
#
# The Rust side of the same contract is vectory-rs/crates/vectory/tests/
# roundtrip_corpus.rs (cargo test), which asserts the Rust serializer
# reproduces these bytes byte-for-byte.

require "leptris"

RSpec.describe "vectory-rs golden corpus parity" do
  repo_root = File.expand_path("../..", __dir__)
  golden_root = File.join(repo_root, "vectory-rs", "testdata", "golden")

  goldens = Dir.glob(File.join(golden_root, "**", "*.svg")).sort
  corpus = Dir.glob(File.join(repo_root, "spec", "**", "*.svg")).sort

  it "has goldens for every parseable corpus file" do
    expect(goldens.size).to eq(52)
    expect(corpus.size).to eq(54)
  end

  goldens.each do |golden|
    rel = golden.delete_prefix("#{golden_root}/")
    source = File.join(repo_root, "spec", rel)

    it "reproduces the Ruby bytes for #{rel}" do
      document = Leptris::XML.parse(File.read(source))
      expect(document.root).not_to(be_nil)
      # Byte comparison in binary encodings: the goldens are checked out
      # verbatim (.gitattributes -text) and to_xml emits LF ends.
      expect(document.root.to_xml.b).to eq(File.binread(golden))
    end
  end
end
