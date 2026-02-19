require "spec_helper"

RSpec.describe Vectory::Emf do
  describe "#to_svg" do
    let(:input)     { "spec/examples/emf2svg/img.emf" }
    let(:reference) { "spec/examples/emf2svg/ref.svg" }

    it "returns svg content" do
      expect(described_class.from_path(input).to_svg.content)
        .to be_xml_equivalent_to File.read(reference)
    end

    it "strips the starting xml tag" do
      expect(described_class.from_path(input).to_svg.content)
        .not_to start_with "<?xml"
    end
  end

  describe "#to_eps" do
    let(:input)     { "spec/examples/emf2eps/img.emf" }
    let(:reference) { "spec/examples/emf2eps/ref.eps" }

    it "returns eps content" do
      expect(described_class.from_path(input).to_eps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#to_ps" do
    let(:input)     { "spec/examples/emf2ps/img.emf" }
    let(:reference) { "spec/examples/emf2ps/ref.ps" }

    it "returns ps content" do
      expect(described_class.from_path(input).to_ps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#mime" do
    let(:input) { "spec/examples/emf2eps/img.emf" }

    it "returns emf" do
      expect(described_class.from_path(input).mime).to eq "image/emf"
    end
  end

  describe "#height" do
    let(:input) { "spec/examples/emf2eps/img.emf" }

    it "returns height" do
      expect(described_class.from_path(input).height).to eq 90
    end
  end

  describe "#width" do
    let(:input) { "spec/examples/emf2eps/img.emf" }

    it "returns width" do
      expect(described_class.from_path(input).width).to eq 90
    end
  end

  describe "::from_node" do
    let(:node) { Nokogiri::XML(File.read(input)).child }
    let(:input) { "spec/examples/emf/datauri.xml" }

    it "can be converted to eps" do
      expect(described_class.from_node(node).to_eps).to be_a(Vectory::Eps)
    end
  end

  describe "error propagation" do
    let(:emf_content) { "fake emf content" }
    let(:emf) { described_class.new(emf_content) }

    context "when Inkscape conversion fails" do
      before do
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")
      end

      it "propagates error from to_eps" do
        expect do
          emf.to_eps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_ps" do
        expect do
          emf.to_ps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end
    end
  end
end
