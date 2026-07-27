require "spec_helper"

RSpec.describe Vectory::Eps do
  shared_examples "converter" do |format|
    it "returns content of a chosen format" do
      to_format_method = "to_#{format}"
      content = described_class.from_path(input)
        .public_send(to_format_method)
        .content

      matcher = case format
                when "eps", "ps" then "be_eps"
                when "svg" then "be_svg"
                when "emf" then "be_emf"
                end

      expect(content)
        .to public_send(matcher, File.read(reference))
    end
  end

  describe "#to_ps" do
    let(:input)     { "spec/examples/eps2ps/img.eps" }
    let(:reference) { "spec/examples/eps2ps/ref.ps" }

    it_behaves_like "converter", "ps"
  end

  describe "#to_svg" do
    let(:input)     { "spec/examples/eps2svg/img.eps" }
    let(:reference) { "spec/examples/eps2svg/ref.svg" }

    it_behaves_like "converter", "svg"
  end

  describe "#to_emf" do
    let(:input)     { "spec/examples/eps2emf/img.eps" }
    let(:reference) { "spec/examples/eps2emf/ref.emf" }

    it_behaves_like "converter", "emf"
  end

  describe "#mime" do
    let(:input) { "spec/examples/eps2emf/img.eps" }

    it "returns postscript" do
      expect(described_class.from_path(input).mime)
        .to eq "application/postscript"
    end
  end

  describe "#height" do
    let(:input) { "spec/examples/eps2emf/img.eps" }

    it "returns height" do
      expect(described_class.from_path(input).height).to eq 720
    end
  end

  describe "#width" do
    let(:input) { "spec/examples/eps2emf/img.eps" }

    it "returns width" do
      expect(described_class.from_path(input).width).to eq 540
    end
  end

  describe "::from_node" do
    let(:node) { Nokogiri::XML(File.read(input)).child }
    let(:input) { "spec/examples/eps/inline.xml" }

    it "can be converted to svg" do
      expect(described_class.from_node(node).to_svg).to be_a(Vectory::Svg)
    end
  end
end
