require "spec_helper"

RSpec.describe Vectory::SvgDocument do
  describe "#remap_links" do
    let(:input) { "spec/examples/svg2eps/img.svg" }

    context "remapped links beforehand" do
      it "converts successfully" do
        document = described_class.new(File.read(input))

        expect { document.remap_links({}) }.not_to raise_error
      end
    end
  end

  describe ".update_ids_css" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:expected_output) { File.read("spec/fixtures/svg-style-attrs-output.svg") }
    let(:document) { Nokogiri::XML(input).root }
    let(:ids) { ["Layer_1", "rect1", "rect2", "grad1"] }
    let(:suffix) { "000000001" }

    it "updates IDs in both style tags and inline style attributes" do
      described_class.update_ids_css(document, ids, suffix)

      # Check that IDs in <style> tag are updated (check with word boundary)
      style_content = document.xpath(".//m:style", "m" => Vectory::SvgDocument::SVG_NS).first.content
      expect(style_content).to match(/#Layer_1_000000001\b/)
      expect(style_content).not_to match(/#Layer_1\b/)

      # Check that IDs in inline style attributes are updated
      # rect1 still has id="rect1" because update_ids_css doesn't update id attributes
      rect1_style = document.xpath(".//m:rect[@id='rect1']", "m" => Vectory::SvgDocument::SVG_NS).first["style"]
      expect(rect1_style).to include("url(#grad1_000000001)")
      expect(rect1_style).not_to match(/url\(\\#grad1\\b\)/)
    end
  end

  describe ".update_ids_attrs" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:document) { Nokogiri::XML(input).root }
    let(:ids) { ["Layer_1", "rect1", "rect2", "grad1"] }
    let(:suffix) { "000000001" }

    it "updates IDs in attributes and url(#id) references" do
      described_class.update_ids_attrs(document, ids, suffix)

      # Check that direct ID attributes are updated
      expect(document["id"]).to eq("Layer_1_000000001")
      expect(document.xpath(".//m:rect[@id='rect1_000000001']", "m" => Vectory::SvgDocument::SVG_NS).first["id"]).to eq("rect1_000000001")
      expect(document.xpath(".//m:rect[@id='rect2_000000001']", "m" => Vectory::SvgDocument::SVG_NS).first["id"]).to eq("rect2_000000001")

      # Check that url(#id) references in attributes are updated
      # rect2 has fill="url(#grad1)" attribute (not inline style)
      fill_attr = document.xpath(".//m:rect[@id='rect2_000000001']", "m" => Vectory::SvgDocument::SVG_NS).first["fill"]
      expect(fill_attr).to eq("url(#grad1_000000001)")
    end
  end

  describe "#suffix_ids" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:expected_output) { File.read("spec/fixtures/svg-style-attrs-output.svg") }
    let(:document) { described_class.new(input) }

    it "updates IDs in style tags, inline styles, and attributes with url(#id)" do
      document.suffix_ids(1)

      # Normalize whitespace for comparison
      result = document.content.gsub(/>\s+</, "><").strip
      expected = expected_output.gsub(/>\s+</, "><").strip

      expect(result).to be_xml_equivalent_to(expected)
    end
  end
end
