# frozen_string_literal: true

# The runtime switch behaviors (lane 3, claricle/vectory#99): auto =
# native-when-loadable, ruby forces the Ruby lane, native demands the
# engine and raises the NAMED error when missing (the etassia#491 law —
# never a silent switch).
#
# No test doubles: the switch is driven through real env values and the
# real library (built locally or in CI's native job). Where the native
# library is not built, those examples are skipped — every other CI job
# proves the Ruby-only path.
require "spec_helper"
require "leptris"

RSpec.describe Vectory::Runtime do
  let(:built_library) { Vectory::Native.library_path }
  let(:missing_library) { File.join("", "nonexistent-vectory-spec", "libvectory_ffi.so") }

  around do |example|
    with_fresh_runtime { example.run }
  end

  describe ".mode" do
    it "defaults to auto" do
      expect(described_class.mode).to eq("auto")
    end

    it "treats an empty value as auto" do
      with_runtime_mode("")
      expect(described_class.mode).to eq("auto")
    end

    described_class::MODES.each do |value|
      it "accepts #{value}" do
        with_runtime_mode(value)
        expect(described_class.mode).to eq(value)
      end
    end

    it "rejects an unknown mode naming the env key and the value" do
      with_runtime_mode("jit")
      expect { described_class.mode }
        .to raise_error(ArgumentError, /VECTORY_RUNTIME.*"jit"/)
    end
  end

  describe ".native?" do
    it "is false when no library exists" do
      with_native_library(missing_library)
      expect(described_class).not_to be_native
    end

    it "probes once and reuses the answer until reset" do
      with_native_library(missing_library)
      expect(described_class).not_to be_native

      # A changed env does not re-open the question in the same process.
      ENV["VECTORY_NATIVE_LIB"] = File.join("", "other-vectory-spec", "libvectory_ffi.so")
      expect(described_class).not_to be_native

      described_class.reset!
      # The re-probe consults the loader afresh: still no library file.
      expect(described_class).not_to be_native
    end

    it "is true when the library is loadable" do
      skip_unless_native_built
      with_native_library(built_library)
      expect(described_class).to be_native
    end

    it "raises the named loader error when the file exists but is not a library" do
      Dir.mktmpdir do |directory|
        impostor = File.join(directory, "libvectory_ffi.dylib")
        File.write(impostor, "not a dylib")
        with_native_library(impostor)

        expect { described_class.native? }
          .to raise_error(Vectory::Native::NativeError, /Cannot load|failed to load/)
      end
    end
  end

  describe ".engine" do
    it "answers :ruby when the library is missing" do
      with_native_library(missing_library)
      expect(described_class.engine).to eq(:ruby)
    end

    it "answers :ruby in ruby mode even when the library is loadable" do
      skip_unless_native_built
      with_native_library(built_library)
      with_runtime_mode("ruby")
      expect(described_class.engine).to eq(:ruby)
    end

    it "answers :native in native mode" do
      skip_unless_native_built
      with_native_library(built_library)
      with_runtime_mode("native")
      expect(described_class.engine).to eq(:native)
    end

    it "answers :native in auto mode when the library is loadable" do
      skip_unless_native_built
      with_native_library(built_library)
      expect(described_class.engine).to eq(:native)
    end
  end

  describe ".roundtrip" do
    let(:svg) { '<svg xmlns="http://www.w3.org/2000/svg" id="shape"><g/></svg>' }
    let(:ruby_reference) { Leptris::XML.parse(svg).root.to_xml }

    describe "in auto mode" do
      it "answers from the Ruby lane when the library is missing" do
        with_native_library(missing_library)
        expect(described_class.roundtrip(svg)).to eq(ruby_reference)
        expect(described_class.engine).to eq(:ruby)
      end

      it "answers from the native engine with the same bytes when it is loadable" do
        skip_unless_native_built
        with_native_library(built_library)
        expect(described_class.roundtrip(svg).b).to eq(ruby_reference.b)
      end
    end

    describe "in ruby mode" do
      it "forces the Ruby lane even when the library is loadable" do
        skip_unless_native_built
        with_native_library(built_library)
        with_runtime_mode("ruby")

        expect(described_class.roundtrip(svg)).to eq(ruby_reference)
      end
    end

    describe "in native mode" do
      it "raises the named error when the engine is missing" do
        with_native_library(missing_library)
        with_runtime_mode("native")

        expect { described_class.roundtrip(svg) }
          .to raise_error(
            described_class::EngineUnavailableError,
            /VECTORY_RUNTIME.*native engine.*not loadable/,
          )
      end

      it "answers from the native engine when it is loadable" do
        skip_unless_native_built
        with_native_library(built_library)
        with_runtime_mode("native")

        expect(described_class.roundtrip(svg).b).to eq(ruby_reference.b)
      end
    end

    describe "with the native-only emfsvg profile" do
      it "refuses with the named error in ruby mode even when loadable" do
        skip_unless_native_built
        with_native_library(built_library)
        with_runtime_mode("ruby")

        expect { described_class.roundtrip(svg, profile: :emfsvg_svg_builder) }
          .to raise_error(
            described_class::EngineUnavailableError,
            /native-only.*no Ruby lane/,
          )
      end

      it "refuses with the named error in auto mode without the library" do
        with_native_library(missing_library)

        expect { described_class.roundtrip(svg, profile: :emfsvg_svg_builder) }
          .to raise_error(described_class::EngineUnavailableError)
      end

      it "answers from the native engine in native mode" do
        skip_unless_native_built
        with_native_library(built_library)
        with_runtime_mode("native")

        expect(described_class.roundtrip(svg, profile: :emfsvg_svg_builder)).to be_a(String)
      end
    end
  end

  describe ".pipeline" do
    let(:steps) { canonical_steps(transform_manifest.fetch("svg_cleanup")) }
    let(:input) { transform_fixture("pipeline_inputs/selected.svg") }

    it "answers from the Ruby lane when the library is missing" do
      with_native_library(missing_library)

      output = described_class.pipeline(input, steps: steps)
      expect(output).to(
        be_xml_equivalent_to(transform_fixture("pipeline_outputs/svg_cleanup.svg")),
      )
    end

    it "answers from the native engine with equivalent bytes when loadable" do
      skip_unless_native_built
      with_native_library(built_library)

      with_runtime_mode("ruby")
      ruby_output = described_class.pipeline(input, steps: steps)

      with_runtime_mode("native")
      expect(described_class.pipeline(input, steps: steps))
        .to(be_xml_equivalent_to(ruby_output))
    end

    it "raises the named error when native mode demands a missing engine" do
      with_native_library(missing_library)
      with_runtime_mode("native")

      expect { described_class.pipeline(input, steps: steps) }
        .to raise_error(described_class::EngineUnavailableError)
    end
  end

  private

  def skip_unless_native_built
    skip "vectory-ffi cdylib not built in this checkout" if built_library.nil?
  end
end
