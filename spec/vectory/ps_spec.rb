require "spec_helper"

RSpec.describe Vectory::Ps do
  describe "#to_eps" do
    let(:input)     { "spec/examples/ps2eps/img.ps" }
    let(:reference) { "spec/examples/ps2eps/ref.eps" }

    it "returns eps content" do
      expect(described_class.from_path(input).to_eps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#to_emf" do
    let(:input)     { "spec/examples/ps2emf/img.ps" }
    let(:reference) { "spec/examples/ps2emf/ref.emf" }

    it "returns emf content" do
      expect(described_class.from_path(input).to_emf.content)
        .to be_emf
    end
  end

  describe "#to_svg" do
    let(:input)     { "spec/examples/ps2svg/img.ps" }
    let(:reference) { "spec/examples/ps2svg/ref.svg" }

    it "returns svg content" do
      expect(described_class.from_path(input).to_svg.content)
        .to be_xml_equivalent_to File.read(reference)
    end
  end

  describe "#mime" do
    let(:input) { "spec/examples/ps2emf/img.ps" }

    it "returns postscript" do
      expect(described_class.from_path(input).mime)
        .to eq "application/postscript"
    end
  end

  describe "#height" do
    let(:input) { "spec/examples/ps2emf/img.ps" }

    it "returns height" do
      expect(described_class.from_path(input).height).to eq 531
    end
  end

  describe "#width" do
    let(:input) { "spec/examples/ps2emf/img.ps" }

    it "returns width" do
      expect(described_class.from_path(input).width).to eq 488
    end
  end

  describe "::from_node" do
    let(:node) { Nokogiri::XML(File.read(input)).child }
    let(:input) { "spec/examples/ps/inline.xml" }

    it "can be converted to svg" do
      expect(described_class.from_node(node).to_svg).to be_a(Vectory::Svg)
    end
  end

  describe "error propagation" do
    let(:ps_content) do
      "%!PS-Adobe-3.0\n%%BoundingBox: 0 0 100 100\n"
    end
    let(:ps) { described_class.new(ps_content) }

    context "when ps2pdf conversion fails" do
      before do
        allow(Vectory::GhostscriptWrapper).to receive(:convert)
          .and_raise(Vectory::ConversionError, "ghostscript failed")
      end

      it "propagates error from to_pdf to to_svg" do
        expect do
          ps.to_svg
        end.to raise_error(Vectory::ConversionError, /ghostscript failed/)
      end

      it "propagates error from to_pdf to to_eps" do
        expect do
          ps.to_eps
        end.to raise_error(Vectory::ConversionError, /ghostscript failed/)
      end

      it "propagates error from to_pdf to to_emf" do
        expect do
          ps.to_emf
        end.to raise_error(Vectory::ConversionError, /ghostscript failed/)
      end
    end

    context "when Inkscape conversion fails" do
      before do
        # Allow ps2pdf to succeed
        allow(Vectory::GhostscriptWrapper).to receive(:convert)
          .and_return("fake pdf content")

        # Make Inkscape fail
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")
      end

      it "propagates error from Inkscape to to_svg" do
        expect do
          ps.to_svg
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end
    end
  end
end
