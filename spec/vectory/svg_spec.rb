require "spec_helper"

RSpec.describe Vectory::Svg do
  describe "#to_emf" do
    let(:input)     { "spec/examples/svg2emf/img.svg" }
    let(:reference) { "spec/examples/svg2emf/ref.emf" }

    it "returns emf content" do
      expect(described_class.from_path(input).to_emf.content)
        .to be_emf
    end
  end

  describe "#to_eps" do
    let(:input)     { "spec/examples/svg2eps/img.svg" }
    let(:reference) { "spec/examples/svg2eps/ref.eps" }

    it "returns eps content" do
      expect(described_class.from_path(input).to_eps.content)
        .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#to_ps" do
    let(:input)     { "spec/examples/svg2ps/img.svg" }
    let(:reference) { "spec/examples/svg2ps/ref.ps" }

    it "returns ps content" do
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
      expect(described_class.from_path(input).height).to eq 100
    end
  end

  describe "#width" do
    let(:input) { "spec/examples/svg2emf/img.svg" }

    it "returns width" do
      expect(described_class.from_path(input).width).to eq 100
    end
  end

  describe "::from_node" do
    let(:node) { Nokogiri::XML(File.read(input)).child }
    let(:input) { "spec/examples/svg/inline.xml" }

    it "can be converted to emf" do
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
end
