require "spec_helper"

RSpec.describe Vectory::Utils do
  # Extracted from https://github.com/metanorma/metanorma-utils/blob/v1.6.5/spec/img_spec.rb

  let(:datauri) { File.read("spec/examples/png/rice_image1.datauri") }

  context "recognises data uris" do
    let(:path) { "spec/examples/png/rice_image1.png" }

    it "where the content is an existing file at a relative path" do
      expect(described_class.datauri(path))
        .to eq described_class.encode_datauri(path)
    end

    it "where the content is an existing file at an absolute path" do
      expect(described_class.datauri(File.expand_path(path)))
        .to eq described_class.encode_datauri(path)
    end

    it "where the content is a relative file path pointing to a bogus file" do
      expect(described_class.datauri("spec/fixtures/bogus.png"))
        .to eq "spec/fixtures/bogus.png"
    end

    it "where the content is an absolute file path pointing to a bogus file" do
      expect(described_class.datauri("D:/spec/fixtures/bogus.png"))
        .to eq "D:/spec/fixtures/bogus.png"
    end

    it "where the content is a data/image URI" do
      expect(described_class.datauri("data1:img/gif,base64,ABBC"))
        .to eq "data1:img/gif,base64,ABBC"
    end

    it "where the content is an URL" do
      expect(described_class.datauri("https://example.com/image.png"))
        .to eq "https://example.com/image.png"
    end
  end

  it "recognises data uris" do
    expect(described_class.datauri?("data:img/gif,base64,ABBC"))
      .to be true
    expect(described_class.datauri?("data1:img/gif,base64,ABBC"))
      .to be false
  end

  it "recognises uris" do
    expect(described_class.url?("mailto://ABC"))
      .to be true
    expect(described_class.url?("http://ABC"))
      .to be true
    expect(described_class.url?("D:/ABC"))
      .to be false
    expect(described_class.url?("/ABC"))
      .to be false
  end

  it "recognises absolute file locations" do
    expect(described_class.absolute_path?("D:/a.html"))
      .to be true
    expect(described_class.absolute_path?("/a.html"))
      .to be true
    expect(described_class.absolute_path?("a.html"))
      .to be false
  end

  context "generates data uris" do
    it "passes datauri untouched" do
      expect(described_class.datauri("data:xyz")).to eq "data:xyz"
    end

    it "returns datauri when path exists" do
      expect(described_class.datauri("spec/examples/png/rice_image1.png"))
        .to be_equivalent_to(datauri)
    end

    it "returns datauri when local path exists" do
      expect(described_class.datauri("rice_image1.png", "spec/examples/png"))
        .to be_equivalent_to(datauri)
    end

    it "passes path if it does not exist" do
      expect(described_class.datauri("spec/fixtures/missing_image.png"))
        .to be_equivalent_to "spec/fixtures/missing_image.png"
    end

    it "prints message if path does not exist" do
      expect(Vectory.ui).to receive(:warn).with(
        "Image specified at `spec/fixtures/rice_image0.png` does not exist.",
      )

      described_class.datauri("spec/fixtures/rice_image0.png")
    end
  end

  describe "#datauri2mime" do
    it "returns mime from datauri input" do
      expect(described_class.datauri2mime(datauri)&.first&.to_s)
        .to eq "image/png"
    end
  end

  describe "#save_dataimage" do
    it "returns tmp path" do
      expect(described_class.save_dataimage(datauri)).to include(Dir.tmpdir)
    end
  end
end
