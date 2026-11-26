# frozen_string_literal: true

# The native ledger (lane 3, claricle/vectory#99): the Rust SVG model and
# the Ruby lane must answer the same corpus with the same output. Runs only
# where the vectory-ffi cdylib is loadable (CI's native-runtime job, or a
# local build); elsewhere it is skipped and the Ruby path is what the rest
# of the suite covers.
#
# Cohorts:
# * roundtrip — the golden corpus, BYTE comparison both lanes (the
#   serializer fidelity contract, pinned by spec/vectory/rs_model_golden_spec.rb).
# * transforms — the manifest's three pipeline profiles, canon comparison
#   both lanes (the lane-2 manipulation parity contract, canon being the
#   recorded equivalence for the transform outputs).
#
# The ledger line warns per cohort, e.g. `native vectory roundtrip: 52
# same (bytes), 0 unported`.
require "spec_helper"
require "leptris"

RSpec.describe Vectory::Runtime do
  let(:built_library) { Vectory::Native.library_path }

  around do |example|
    with_fresh_runtime do
      with_native_library(built_library)
      example.run
    end
  end

  it "roundtrips the golden corpus byte-identically through Rust and Ruby" do
    skip_unless_native_built
    repo_root = File.expand_path("../..", __dir__)
    golden_root = File.join(repo_root, "vectory-rs", "testdata", "golden")
    goldens = Dir.glob(File.join(golden_root, "**", "*.svg"))
    expect(goldens).not_to be_empty

    same = 0
    different = []
    goldens.each do |golden|
      rel = golden.delete_prefix("#{golden_root}/")
      content = File.read(File.join(repo_root, "spec", rel))
      ruby_bytes = Leptris::XML.parse(content).root.to_xml.b
      native_bytes = described_class.roundtrip(content, profile: :ruby_xml).to_s.b

      if ruby_bytes == native_bytes
        same += 1
      else
        different << rel
      end
    end

    warn "native vectory roundtrip: #{same} same (bytes), " \
         "#{different.size} unported"
    expect(different).to eq([])
  end

  %w[graft svg_cleanup graft_then_svg_cleanup].each do |name|
    it "executes the #{name} profile identically through Rust and Ruby" do
      skip_unless_native_built
      contract = transform_manifest.fetch(name)
      ruby_output = transform_output(name, "ruby")
      native_output = transform_output(name, "native")

      matcher = be_xml_equivalent_to(ruby_output)
      same = matcher.matches?(native_output)
      warn "native vectory transforms (#{name}): " \
           "#{same ? 1 : 0} same (canon), #{same ? 0 : 1} unported"

      expect(native_output).to(matcher)
      expect(native_output)
        .to(be_xml_equivalent_to(transform_fixture(contract.fetch("output"))))
    end
  end

  private

  # Canon comparison: the recorded parity contract for the transform
  # outputs (spec/vectory/svg_transform_integration_spec.rb's
  # be_xml_equivalent_to, mirrored by the Rust manip tests).
  def transform_output(name, mode)
    contract = transform_manifest.fetch(name)
    with_runtime_mode(mode)
    described_class.pipeline(
      transform_fixture(contract.fetch("input")),
      steps: canonical_steps(contract),
    )
  end

  def skip_unless_native_built
    skip "vectory-ffi cdylib not built in this checkout" if built_library.nil?
  end
end
