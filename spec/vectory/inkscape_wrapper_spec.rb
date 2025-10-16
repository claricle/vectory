require "spec_helper"

RSpec.describe Vectory::InkscapeWrapper do
  describe "#convert" do
    context "file has inproper format: svg extension, but eps content" do
      let(:input) { "spec/examples/eps_but_svg_extension.svg" }

      it "raises error" do
        content = File.read(input, mode: "rb")

        expect do
          described_class.convert(
            content: content,
            input_format: :svg,
            output_format: :emf,
            output_class: Vectory::Emf,
          )
        end.to raise_error(Vectory::ConversionError,
                           /parser error : Start tag expected/)
      end
    end

    context "inkscape is not installed" do
      let(:input) { "spec/examples/eps2svg/img.eps" }

      it "raises error" do
        content = File.read(input, mode: "rb")

        expect(described_class.instance)
          .to receive(:inkscape_path).and_return(nil)

        expect do
          described_class.convert(
            content: content,
            input_format: :eps,
            output_format: :svg,
            output_class: Vectory::Svg,
          )
        end.to raise_error(Vectory::InkscapeNotFoundError)
      end
    end
  end
end
