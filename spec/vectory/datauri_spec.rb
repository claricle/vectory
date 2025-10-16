require "spec_helper"

RSpec.describe Vectory::Datauri do
  describe "::from_vector" do
    context "eps" do
      let(:input) { "spec/examples/eps2emf/img.eps" }
      let(:vector) { Vectory::Eps.from_path(input) }

      it "returns datauri content" do
        expect(Vectory::Datauri.from_vector(vector).content)
          .to eq File.read("#{input}.datauri")
      end
    end

    context "ps" do
      let(:input) { "spec/examples/ps2emf/img.ps" }
      let(:vector) { Vectory::Ps.from_path(input) }

      it "returns datauri content" do
        expect(Vectory::Datauri.from_vector(vector).content)
          .to eq File.read("#{input}.datauri")
      end
    end

    context "emf" do
      let(:input) { "spec/examples/emf2eps/img.emf" }
      let(:vector) { Vectory::Emf.from_path(input) }

      it "returns datauri content" do
        expect(Vectory::Datauri.from_vector(vector).content)
          .to eq File.read("#{input}.datauri")
      end
    end

    describe "svg" do
      let(:input) { "spec/examples/svg2emf/img.svg" }
      let(:vector) { Vectory::Svg.from_path(input) }

      it "returns datauri content" do
        expect(Vectory::Datauri.from_vector(vector).content)
          .to eq File.read("#{input}.datauri")
      end
    end
  end

  describe "#to_vector" do
    context "incorrect data" do
      let(:not_a_datauri) { "123abc" }

      it "raises conversion error" do
        expect { Vectory::Datauri.new(not_a_datauri).to_vector }
          .to raise_error(Vectory::ConversionError)
      end
    end

    context "eps" do
      let(:input)     { "spec/examples/eps2emf/img.eps.datauri" }
      let(:reference) { "spec/examples/eps2emf/img.eps" }

      it "returns eps content" do
        expect(Vectory::Datauri.from_path(input).to_vector.content)
          .to eq File.read(reference)
      end

      it "returns Eps class" do
        expect(Vectory::Datauri.from_path(input).to_vector)
          .to be_kind_of(Vectory::Eps)
      end
    end

    context "emf" do
      let(:input)     { "spec/examples/emf2eps/img.emf.datauri" }
      let(:reference) { "spec/examples/emf2eps/img.emf" }

      it "returns emf content" do
        expect(Vectory::Datauri.from_path(input).to_vector.content)
          .to eq File.binread(reference)
      end

      it "returns Emf class" do
        expect(Vectory::Datauri.from_path(input).to_vector)
          .to be_kind_of(Vectory::Emf)
      end
    end

    context "svg" do
      let(:input)     { "spec/examples/svg2emf/img.svg.datauri" }
      let(:reference) { "spec/examples/svg2emf/img.svg" }

      it "returns svg content" do
        expect(Vectory::Datauri.from_path(input).to_vector.content)
          .to be_equivalent_to File.read(reference)
      end

      it "returns Svg class" do
        expect(Vectory::Datauri.from_path(input).to_vector)
          .to be_kind_of(Vectory::Svg)
      end
    end
  end

  describe "#mime" do
    context "incorrect data" do
      let(:not_a_datauri) { "123abc" }

      it "raises conversion error" do
        expect { described_class.new(not_a_datauri).mime }
          .to raise_error(Vectory::ConversionError)
      end
    end

    context "eps" do
      let(:input) { "spec/examples/eps2emf/img.eps.datauri" }

      it "returns eps" do
        expect(described_class.from_path(input).mime)
          .to eq "application/postscript"
      end
    end

    context "emf" do
      let(:input) { "spec/examples/emf2eps/img.emf.datauri" }

      it "returns emf" do
        expect(described_class.from_path(input).mime).to eq "image/emf"
      end
    end

    context "svg" do
      let(:input) { "spec/examples/svg2emf/img.svg.datauri" }

      it "returns svg" do
        expect(described_class.from_path(input).mime).to eq "image/svg+xml"
      end
    end
  end

  describe "dimensions (#height / #width)" do
    context "incorrect data" do
      let(:not_a_datauri) { "123abc" }

      it "raises conversion error" do
        expect { described_class.new(not_a_datauri).height }
          .to raise_error(Vectory::ConversionError)

        expect { described_class.new(not_a_datauri).width }
          .to raise_error(Vectory::ConversionError)
      end
    end

    context "eps" do
      let(:input) { "spec/examples/eps2emf/img.eps.datauri" }

    it "returns height and width" do
      expect(described_class.from_path(input).height)
        .to eq 720

        expect(described_class.from_path(input).width)
          .to eq 540
      end
    end

    context "emf" do
      let(:input) { "spec/examples/emf2eps/img.emf.datauri" }

      it "returns height and width" do
        expect(described_class.from_path(input).height).to eq 90
        expect(described_class.from_path(input).width).to eq 90
      end
    end

    context "svg" do
      let(:input) { "spec/examples/svg2emf/img.svg.datauri" }

      it "returns height and width" do
        expect(described_class.from_path(input).height).to eq 90
        expect(described_class.from_path(input).width).to eq 90
      end
    end
  end
  describe "round-trip integrity" do
    context "emf" do
      let(:input_path) { "spec/examples/emf2eps/img.emf" }

      it "preserves EMF content through Data URI round-trip" do
        original = Vectory::Emf.from_path(input_path)
        datauri1 = Vectory::Datauri.from_vector(original)
        restored = datauri1.to_vector
        datauri2 = Vectory::Datauri.from_vector(restored)

        expect(datauri2.content).to eq datauri1.content
      end
    end
  end
end
