require "spec_helper"

RSpec.describe Vectory::FileMagic do
  describe "#detect" do
    references = {
      "spec/examples/eps2svg/img.eps" => :eps,
      "spec/examples/eps2emf/img.eps" => :eps,
      "spec/examples/eps2svg/ref.svg" => :svg,
      "spec/examples/svg2emf/img.svg" => :svg,
      "spec/examples/eps2emf/ref.emf" => :emf,
      "spec/examples/svg2emf/ref.emf" => :emf,
    }

    it "returns nil for an empty file" do
      Dir.mktmpdir do |dir|
        path = File.join(dir, "empty.svg")
        File.binwrite(path, "")
        expect(described_class.detect(path)).to be_nil
      end
    end

    references.each do |file, format|
      context file do
        it "returns #{format} format" do
          expect(described_class.detect(file)).to eq format
        end
      end
    end
  end
end
