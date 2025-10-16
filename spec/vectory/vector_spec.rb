require "spec_helper"

RSpec.describe Vectory::Vector do
  describe "#to_uri" do
    let(:input) { "spec/examples/eps2emf/img.eps" }
    let(:vector) { Vectory::Eps.from_path(input) }

    it "returns Datauri object" do
      expect(vector.to_uri).to be_a(Vectory::Datauri)
    end
  end

  describe "#write" do
    let(:image) { Vectory::Emf.from_path("spec/examples/emf2svg/img.emf") }

    context "no arg" do
      it "saves to a temporary directory" do
        expect(image.write.path).to include(Dir.tmpdir)
      end
    end

    context "path provided" do
      it "saves to the provided path" do
        Dir.mktmpdir do |tmp_dir|
          provided_path = File.join(tmp_dir, "image.emf")
          expect(image.write(provided_path).path).to eq provided_path
        end
      end
    end

    context "created from path and no arg provided" do
      it "saves to a temporary directory" do
        expect(image.write.path).to include(Dir.tmpdir)
      end
    end

    context "written two times without arg" do
      it "writes to the same path" do
        initial_path = image.write.path
        expect(image.write.path).to eq initial_path
      end
    end

    context "after conversion and no arg" do
      it "saves to a temporary directory" do
        expect(image.to_svg.write.path).to include(Dir.tmpdir)
      end
    end
  end

  describe "#path" do
    let(:image) { Vectory::Emf.from_path("spec/examples/emf2svg/img.emf") }

    context "created from path" do
      it "raises error that it is not written to disk" do
        expect { image.path }.to raise_error(Vectory::NotWrittenToDiskError)
      end
    end

    context "written to disk" do
      before { image.write }

      it "returns path on a disk" do
        expect(image.path).to include(Dir.tmpdir)
      end
    end
  end

  describe "#size" do
    let(:image) { Vectory::Emf.from_path("spec/examples/emf2svg/img.emf") }

    it "returns content size" do
      expect(image.size).to eq 1060
    end
  end

  describe "#file_size" do
    let(:image) { Vectory::Eps.from_path("spec/examples/eps2emf/img.eps") }

    context "not written to disk" do
      it "raises not-written error" do
        expect { image.file_size }
          .to raise_error(Vectory::NotWrittenToDiskError)
      end
    end

    context "written to disk" do
      before { image.write }

      it "returns file size" do
        expect(image.file_size).to satisfy("be either 2926 or 3125") do |v|
          [2926, 3125].include?(v) # depends on file system
        end
      end
    end
  end
end
