require "spec_helper"
require "vectory/cli"

RSpec.describe Vectory::CLI do
  before do
    # Vectory.ui.level = :debug
    Vectory.ui.level = ENV["VECTORY_LOG"] || :fatal
  end

  describe "#convert" do
    shared_examples "converter" do |format|
      it "creates file of a chosen format" do
        matcher = case format
                  when "eps", "ps" then "be_eps"
                  when "svg" then "be_svg"
                  when "emf" then "be_emf"
                  end

        with_tmp_dir do |dir|
          output = File.join(dir, "output.#{format}")
          status = described_class.start(["-f", format, "-o", output, input])
          expect(status).to be Vectory::CLI::STATUS_SUCCESS
          expect(File.read(output))
            .to public_send(matcher, File.read(reference))
        end
      end
    end

    context "eps to ps" do
      let(:input)     { "spec/examples/eps2ps/img.eps" }
      let(:reference) { "spec/examples/eps2ps/ref.ps" }

      it_behaves_like "converter", "ps"
    end

    context "eps to svg" do
      let(:input)     { "spec/examples/eps2svg/img.eps" }
      let(:reference) { "spec/examples/eps2svg/ref.svg" }

      it_behaves_like "converter", "svg"
    end

    context "eps to emf" do
      let(:input)     { "spec/examples/eps2emf/img.eps" }
      let(:reference) { "spec/examples/eps2emf/ref.emf" }

      it_behaves_like "converter", "emf"
    end

    context "ps to eps" do
      let(:input)     { "spec/examples/ps2eps/img.ps" }
      let(:reference) { "spec/examples/ps2eps/ref.eps" }

      it_behaves_like "converter", "eps"
    end

    context "ps to emf" do
      let(:input)     { "spec/examples/ps2emf/img.ps" }
      let(:reference) { "spec/examples/ps2emf/ref.emf" }

      it_behaves_like "converter", "emf"
    end

    context "ps to svg" do
      let(:input)     { "spec/examples/ps2svg/img.ps" }
      let(:reference) { "spec/examples/ps2svg/ref.svg" }

      it_behaves_like "converter", "svg"
    end

    context "svg to emf" do
      let(:input)     { "spec/examples/svg2emf/img.svg" }
      let(:reference) { "spec/examples/svg2emf/ref.emf" }

      it_behaves_like "converter", "emf"
    end

    context "svg to eps" do
      let(:input)     { "spec/examples/svg2eps/img.svg" }
      let(:reference) { "spec/examples/svg2eps/ref.eps" }

      it_behaves_like "converter", "eps"
    end

    context "svg to ps" do
      let(:input)     { "spec/examples/svg2ps/img.svg" }
      let(:reference) { "spec/examples/svg2ps/ref.ps" }

      it_behaves_like "converter", "ps"
    end

    context "emf to svg" do
      let(:input)     { "spec/examples/emf2svg/img.emf" }
      let(:reference) { "spec/examples/emf2svg/ref.svg" }

      it_behaves_like "converter", "svg"
    end

    context "emf to eps" do
      let(:input)     { "spec/examples/emf2eps/img.emf" }
      let(:reference) { "spec/examples/emf2eps/ref.eps" }

      it_behaves_like "converter", "eps"
    end

    context "emf to ps" do
      let(:input)     { "spec/examples/emf2ps/img.emf" }
      let(:reference) { "spec/examples/emf2ps/ref.ps" }

      it_behaves_like "converter", "ps"
    end

    context "jpg to svg" do
      let(:input)  { "spec/examples/img.jpg" }
      let(:format) { "svg" }

      it "returns unsupported-input-format error" do
        with_tmp_dir do |dir|
          expect(Vectory.ui).to receive(:error)
            .with(start_with("Could not detect input format. " \
                             "Please provide file of the following formats:"))

          output = File.join(dir, "output.#{format}")
          status = described_class.start(["-f", format, "-o", output, input])

          expect(status)
            .to be Vectory::CLI::STATUS_UNSUPPORTED_INPUT_FORMAT_ERROR
        end
      end
    end

    context "svg to jpg" do
      let(:input)  { "spec/examples/svg2emf/img.svg" }
      let(:format) { "jpg" }

      it "returns unsupported-output-format error" do
        with_tmp_dir do |dir|
          expect(Vectory.ui).to receive(:error)
            .with(start_with("Unsupported output format '#{format}'. " \
                             "Please choose one of:"))

          output = File.join(dir, "output.#{format}")
          status = described_class.start(["-f", format, "-o", output, input])

          expect(status)
            .to be Vectory::CLI::STATUS_UNSUPPORTED_OUTPUT_FORMAT_ERROR
        end
      end
    end

    context "emf to emf (same type)" do
      let(:input)  { "spec/examples/emf2svg/img.emf" }
      let(:format) { "emf" }

      it "returns same-format error" do
        with_tmp_dir do
          expect(Vectory.ui).to receive(:error)
            .with("Could not convert to the same format '#{format}'.")

          status = described_class.start(["-f", format, input])

          expect(status).to be Vectory::CLI::STATUS_SAME_FORMAT_ERROR
        end
      end
    end

    context "inkscape throws ConversionError" do
      let(:input)  { "spec/examples/eps2emf/img.eps" }
      let(:format) { "emf" }

      it "returns conversion error" do
        with_tmp_dir do |dir|
          expect(Vectory::InkscapeWrapper.instance).to receive(:convert)
            .and_raise(Vectory::ConversionError)

          output = File.join(dir, "output.#{format}")
          status = described_class.start(["-f", format, "-o", output, input])

          expect(status).to be Vectory::CLI::STATUS_CONVERSION_ERROR
        end
      end
    end

    context "got SystemCallError" do
      let(:input)  { "spec/examples/eps2emf/img.eps" }
      let(:format) { "emf" }

      it "returns conversion error" do
        with_tmp_dir do |dir|
          # Since we're now using Ukiryu directly, mock GhostscriptWrapper to raise ConversionError
          expect(Vectory::GhostscriptWrapper).to receive(:convert)
            .and_raise(Vectory::ConversionError, "Conversion failed")

          output = File.join(dir, "output.#{format}")
          status = described_class.start(["-f", format, "-o", output, input])

          # ConversionError from GhostscriptWrapper should return STATUS_CONVERSION_ERROR
          expect(status).to be Vectory::CLI::STATUS_CONVERSION_ERROR
        end
      end
    end

    context "got InkscapeNotFoundError" do
      let(:input)  { Vectory.root_path.join("spec/examples/eps2emf/img.eps") }
      let(:format) { "emf" }

      it "returns inkscape-not-found error" do
        in_tmp_dir do
          expect(Vectory::InkscapeWrapper.instance)
            .to receive(:convert).and_raise(Vectory::InkscapeNotFoundError)

          status = described_class.start(["-f", format, input])

          expect(status).to be Vectory::CLI::STATUS_INKSCAPE_NOT_FOUND_ERROR
        end
      end
    end

    context "got GhostscriptNotFoundError" do
      let(:input)  { Vectory.root_path.join("spec/examples/eps2emf/img.eps") }
      let(:format) { "emf" }

      it "returns ghostscript-not-found error" do
        in_tmp_dir do
          expect(Vectory::GhostscriptWrapper)
            .to receive(:convert).and_raise(Vectory::GhostscriptNotFoundError)

          status = described_class.start(["-f", format, input])

          expect(status).to be Vectory::CLI::STATUS_GHOSTSCRIPT_NOT_FOUND_ERROR
        end
      end
    end

    context "no output option (it's not required)" do
      let(:input)  { Vectory.root_path.join("spec/examples/eps2emf/img.eps") }
      let(:format) { "emf" }

      it "uses input filename with a new extension and writes to current dir" do
        in_tmp_dir do |dir|
          status = described_class.start(["-f", format, input])

          expect(Pathname.new(File.join(dir, "img.emf"))).to exist
          expect(status).to be Vectory::CLI::STATUS_SUCCESS
        end
      end
    end
  end
end
