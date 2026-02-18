require "spec_helper"

# NOTE: Fixtures can be regenerated using external tools (Ghostscript, Inkscape, cairo):
# bundle exec ruby spec/support/fixture_generator.rb

RSpec.describe Vectory::Ps do
  describe "#to_eps" do
    let(:input)     { "spec/examples/ps2eps/img.ps" }
    # let(:reference) { "spec/examples/ps2eps/ref.eps" }

    it "returns eps content" do
      skip_inkscape_on_windows
      expect(described_class.from_path(input).to_eps.content)
        .to be_eps
      # Commented out due to Cairo version differences between environments
      # expect(Vectory::Ps.from_path(input).to_eps.content)
      #   .to be_equivalent_eps_to File.read(reference)
    end
  end

  describe "#to_emf" do
    let(:input)     { "spec/examples/ps2emf/img.ps" }
    let(:reference) { "spec/examples/ps2emf/ref.emf" }

    it "returns emf content" do
      skip_inkscape_on_windows
      expect(described_class.from_path(input).to_emf.content)
        .to be_emf
    end
  end

  describe "#to_svg" do
    let(:input)     { "spec/examples/ps2svg/img.ps" }
    # let(:reference) { "spec/examples/ps2svg/ref.svg" }

    it "returns svg content" do
      skip_inkscape_on_windows
      expect(described_class.from_path(input).to_svg.content)
        .to be_svg
      # Commented out due to Cairo version differences between environments
      # expect(Vectory::Ps.from_path(input).to_svg.content)
      #   .to be_equivalent_xml_to File.read(reference)
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
      skip_inkscape_on_windows
      expect(described_class.from_node(node).to_svg).to be_a(Vectory::Svg)
    end
  end
end
