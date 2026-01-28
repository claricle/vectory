require "spec_helper"

RSpec.describe Vectory::Pdf do
  describe "error propagation" do
    let(:pdf_content) { "fake pdf content" }
    let(:pdf) { described_class.new(pdf_content) }

    context "when Inkscape conversion fails and fallback also fails" do
      before do
        # Mock Inkscape to fail
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")

        # Mock Ghostscript fallback to also fail
        allow(Vectory::GhostscriptWrapper).to receive(:pdf_to_eps)
          .and_raise(Vectory::ConversionError, "Ghostscript fallback failed")
      end

      it "propagates error from to_svg" do
        expect do
          pdf.to_svg
        end.to raise_error(Vectory::ConversionError, /Ghostscript fallback failed/)
      end

      it "propagates error from to_eps" do
        expect do
          pdf.to_eps
        end.to raise_error(Vectory::ConversionError, /Ghostscript fallback failed/)
      end

      it "propagates error from to_ps" do
        expect do
          pdf.to_ps
        end.to raise_error(Vectory::ConversionError, /Ghostscript fallback failed/)
      end

      it "propagates error from to_emf" do
        expect do
          pdf.to_emf
        end.to raise_error(Vectory::ConversionError, /Ghostscript fallback failed/)
      end
    end

    context "when Inkscape conversion fails but fallback succeeds" do
      before do
        # Mock Inkscape to fail
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")

        # Mock Ghostscript fallback to succeed and return valid EPS content
        allow(Vectory::GhostscriptWrapper).to receive(:pdf_to_eps)
          .and_return("%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 100 100\n")

        # Mock the second Inkscape call (EPS -> target) to succeed
        # For SVG conversion
        svg_output = Vectory::Svg.new("<svg></svg>")
        allow(converter).to receive(:convert)
          .with(
            hash_including(input_format: :eps, output_format: :svg, plain: true)
          )
          .and_return(svg_output)

        # For EPS conversion
        eps_output = Vectory::Eps.new("%!PS-Adobe-3.0 EPSF-3.0")
        allow(converter).to receive(:convert)
          .with(
            hash_including(input_format: :eps, output_format: :eps)
          )
          .and_return(eps_output)

        # For PS conversion
        ps_output = Vectory::Ps.new("%!PS-Adobe-3.0")
        allow(converter).to receive(:convert)
          .with(
            hash_including(input_format: :eps, output_format: :ps)
          )
          .and_return(ps_output)

        # For EMF conversion
        emf_output = Vectory::Emf.new("\x01\x00\x00\x00")
        allow(converter).to receive(:convert)
          .with(
            hash_including(input_format: :eps, output_format: :emf)
          )
          .and_return(emf_output)
      end

      it "succeeds via fallback for to_svg" do
        expect(pdf.to_svg).to be_a(Vectory::Svg)
      end

      it "succeeds via fallback for to_eps" do
        expect(pdf.to_eps).to be_a(Vectory::Eps)
      end

      it "succeeds via fallback for to_ps" do
        expect(pdf.to_ps).to be_a(Vectory::Ps)
      end

      it "succeeds via fallback for to_emf" do
        expect(pdf.to_emf).to be_a(Vectory::Emf)
      end
    end
  end
end
