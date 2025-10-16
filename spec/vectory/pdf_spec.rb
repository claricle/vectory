require "spec_helper"

RSpec.describe Vectory::Pdf do
  describe "error propagation" do
    let(:pdf_content) { "fake pdf content" }
    let(:pdf) { described_class.new(pdf_content) }

    context "when Inkscape conversion fails" do
      before do
        converter = instance_double(Vectory::InkscapeWrapper)
        allow(Vectory::InkscapeWrapper).to receive(:instance).and_return(converter)
        allow(converter).to receive(:convert)
          .and_raise(Vectory::ConversionError, "Inkscape failed")
      end

      it "propagates error from to_svg" do
        expect do
          pdf.to_svg
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_eps" do
        expect do
          pdf.to_eps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_ps" do
        expect do
          pdf.to_ps
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end

      it "propagates error from to_emf" do
        expect do
          pdf.to_emf
        end.to raise_error(Vectory::ConversionError, /Inkscape failed/)
      end
    end
  end
end
