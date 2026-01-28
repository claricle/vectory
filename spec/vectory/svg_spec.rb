require "spec_helper"

RSpec.describe Vectory::Svg do
  describe "#to_emf" do
    let(:input)     { "spec/examples/svg2emf/img.svg" }
    let(:reference) { "spec/examples/svg2emf/ref.emf" }

    it "returns emf content" do
      skip_emf_on_windows
      expect(described_class.from_path(input).to_emf.content)
        .to be_emf
    end
  end

  describe "#to_eps" do
    let(:input)     { "spec/examples/svg2eps/img.svg" }
    let(:reference) { "spec/examples/svg2eps/ref.eps" }

    it "returns eps content" do
      skip_inkscape_on_windows
      expect(described_class.from_path(input).to_eps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#to_ps" do
    let(:input)     { "spec/examples/svg2ps/img.svg" }
    let(:reference) { "spec/examples/svg2ps/ref.ps" }

    it "returns ps content" do
      skip_inkscape_on_windows
      expect(described_class.from_path(input).to_ps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#mime" do
    let(:input) { "spec/examples/svg2emf/img.svg" }

    it "returns svg" do
      expect(described_class.from_path(input).mime).to eq "image/svg+xml"
    end
  end

  describe "#height" do
    let(:input) { "spec/examples/svg2emf/img.svg" }

    it "returns height" do
      expect(described_class.from_path(input).height).to eq 90
    end
  end

  describe "#width" do
    let(:input) { "spec/examples/svg2emf/img.svg" }

    it "returns width" do
      expect(described_class.from_path(input).width).to eq 90
    end
  end

  describe "::from_node" do
    let(:node) { Nokogiri::XML(File.read(input)).child }
    let(:input) { "spec/examples/svg/inline.xml" }

    it "can be converted to emf" do
      skip_emf_on_windows
      expect(described_class.from_node(node).to_emf).to be_a(Vectory::Emf)
    end
  end

  describe "::new" do
    context "incorrect data" do
      let(:command) { described_class.new("incorrect123svg") }

      it "raises parsing error" do
        expect { command }.to raise_error(Vectory::ParsingError)
      end
    end
  end

  describe "error propagation" do
    let(:svg_content) { '<svg xmlns="http://www.w3.org/2000/svg"><rect/></svg>' }
    let(:svg) { described_class.new(svg_content) }

    context "when Inkscape conversion fails" do
      before do
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")
      end

      it "propagates error from to_emf" do
        expect do
          svg.to_emf
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_eps" do
        expect do
          svg.to_eps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_ps" do
        expect do
          svg.to_ps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end
    end
  end
end
