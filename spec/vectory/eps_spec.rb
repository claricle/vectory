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

    context "incorrect data" do
      let(:command) { described_class.from_content("incorrect123") }

      it "raises query error" do
        expect { command.height }.to raise_error(Vectory::InkscapeQueryError)
      end
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

  describe "error propagation" do
    let(:eps_content) do
      "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 100 100\n"
    end
    let(:eps) { described_class.new(eps_content) }

    context "when ps2pdf conversion fails" do
      before do
        allow(Vectory::GhostscriptWrapper).to receive(:convert)
          .and_raise(Vectory::ConversionError, "ghostscript failed")
      end

      it "propagates error from to_pdf to to_svg" do
        expect do
          eps.to_svg
        end.to raise_error(Vectory::ConversionError, /ghostscript failed/)
      end

      it "propagates error from to_pdf to to_ps" do
        expect do
          eps.to_ps
        end.to raise_error(Vectory::ConversionError, /ghostscript failed/)
      end

      it "propagates error from to_pdf to to_emf" do
        expect do
          eps.to_emf
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
          eps.to_svg
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end
    end
  end
end
