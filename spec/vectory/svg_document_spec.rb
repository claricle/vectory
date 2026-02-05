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
    let(:expected_output) do
      File.read("spec/fixtures/svg-style-attrs-output.svg")
    end
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
      expect(document.xpath(".//m:rect[@id='rect1_000000001']",
                            "m" => Vectory::SvgDocument::SVG_NS).first["id"]).to eq("rect1_000000001")
      expect(document.xpath(".//m:rect[@id='rect2_000000001']",
                            "m" => Vectory::SvgDocument::SVG_NS).first["id"]).to eq("rect2_000000001")

      # Check that url(#id) references in attributes are updated
      # rect2 has fill="url(#grad1)" attribute (not inline style)
      fill_attr = document.xpath(".//m:rect[@id='rect2_000000001']", "m" => Vectory::SvgDocument::SVG_NS).first["fill"]
      expect(fill_attr).to eq("url(#grad1_000000001)")
    end
  end

  describe "#suffix_ids" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:expected_output) do
      File.read("spec/fixtures/svg-style-attrs-output.svg")
    end
    let(:document) { described_class.new(input) }

    it "updates IDs in style tags, inline styles, and attributes with url(#id)" do
      document.suffix_ids(1)

      # Normalize whitespace for comparison
      result = document.content.gsub(/>\s+</, "><").strip
      expected = expected_output.gsub(/>\s+</, "><").strip

      expect(result).to be_xml_equivalent_to(expected)
    end
  end

  describe "#apply_id_suffix" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:document) { described_class.new(input) }
    # The code prepends an underscore, so pass suffix without leading underscore
    let(:id_suffix) { "ISO_17301-1_2016" }

    it "applies ID suffix for cross-document uniqueness" do
      document.apply_id_suffix(id_suffix)

      # Check that ID attributes are suffixed (code adds underscore: _suffix)
      expect(document.content).to match(/id="Layer_1_ISO_17301-1_2016"/)
      expect(document.content).to match(/id="rect1_ISO_17301-1_2016"/)
      expect(document.content).to match(/id="rect2_ISO_17301-1_2016"/)
      expect(document.content).to match(/id="grad1_ISO_17301-1_2016"/)

      # Check that CSS references are updated
      expect(document.content).to match(/#Layer_1_ISO_17301-1_2016\b/)
      expect(document.content).to match(/url\(#grad1_ISO_17301-1_2016\)/)

      # Check original IDs no longer exist (with word boundary)
      expect(document.content).not_to match(/#Layer_1\b/)
    end

    it "returns self for chaining" do
      result = document.apply_id_suffix(id_suffix)
      expect(result).to eq(document)
    end

    context "when document has no IDs" do
      let(:input) { "<svg xmlns='http://www.w3.org/2000/svg'><rect x='0' y='0' width='10' height='10'/></svg>" }

      it "does not modify the document" do
        original_content = document.content
        document.apply_id_suffix(id_suffix)
        expect(document.content).to eq(original_content)
      end
    end
  end

  describe "#apply_index_suffix" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:document) { described_class.new(input) }

    it "applies zero-padded 9-digit index suffix" do
      document.apply_index_suffix(1)

      # Check that index is zero-padded to 9 digits
      expect(document.content).to match(/id="Layer_1_000000001"/)
      expect(document.content).to match(/id="rect1_000000001"/)
      expect(document.content).to match(/id="rect2_000000001"/)
      expect(document.content).to match(/id="grad1_000000001"/)
    end

    it "applies different index values correctly" do
      document.apply_index_suffix(42)

      expect(document.content).to match(/id="Layer_1_000000042"/)
      expect(document.content).to match(/id="rect1_000000042"/)
    end

    it "accepts string suffix directly (code prepends underscore for strings too)" do
      # String suffix also gets underscore prepended by the code
      document.apply_index_suffix("custom_suffix")

      expect(document.content).to match(/id="Layer_1_custom_suffix"/)
      expect(document.content).to match(/id="rect1_custom_suffix"/)
    end

    it "returns self for chaining" do
      result = document.apply_index_suffix(1)
      expect(result).to eq(document)
    end
  end

  describe "#namespace" do
    let(:input) { File.read("spec/fixtures/svg-style-attrs-input.svg") }
    let(:document) { described_class.new(input) }
    let(:links_map) { {} }
    # Use a valid xpath that matches nothing (no processing instructions in the fixture)
    let(:xpath_to_remove) { "processing-instruction()" }

    context "with only index suffix" do
      it "applies only the index suffix" do
        document.namespace(1, links_map, xpath_to_remove)

        expect(document.content).to match(/id="Layer_1_000000001"/)
        expect(document.content).to match(/id="rect1_000000001"/)
      end
    end

    context "with both id_suffix and index suffix" do
      # Pass suffix without leading underscore - code adds it
      let(:id_suffix) { "DOC_1" }

      it "applies both suffixes in correct order" do
        document.namespace(2, links_map, xpath_to_remove, id_suffix: id_suffix)

        # Final format: <original_id>_<id_suffix>_<index_suffix>
        expect(document.content).to match(/id="Layer_1_DOC_1_000000002"/)
        expect(document.content).to match(/id="rect1_DOC_1_000000002"/)
        expect(document.content).to match(/id="rect2_DOC_1_000000002"/)
        expect(document.content).to match(/id="grad1_DOC_1_000000002"/)
      end

      it "updates CSS references with both suffixes" do
        document.namespace(5, links_map, xpath_to_remove, id_suffix: id_suffix)

        expect(document.content).to match(/#Layer_1_DOC_1_000000005\b/)
        expect(document.content).to match(/url\(#grad1_DOC_1_000000005\)/)
      end
    end

    context "with complex id_suffix (ISO identifier)" do
      let(:id_suffix) { "ISO_17301-1_2016" }

      it "correctly applies the ISO-based suffix" do
        document.namespace(0, links_map, xpath_to_remove, id_suffix: id_suffix)

        expect(document.content).to match(/id="Layer_1_ISO_17301-1_2016_000000000"/)
        expect(document.content).to match(/id="rect1_ISO_17301-1_2016_000000000"/)
      end
    end

    context "with link remapping" do
      let(:links_map) do
        # File.expand_path is used on both map keys and lookups
        # so we need to use the expanded paths as keys
        {
          File.expand_path("mn://action_schema") => "#ref1",
          File.expand_path("http://example.com") => "#external_ref",
        }
      end
      let(:input) do
        <<~SVG
          <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
            <a xlink:href="mn://action_schema"><rect x="0" y="0" width="10" height="10"/></a>
            <a xlink:href="http://example.com"><rect x="20" y="20" width="10" height="10"/></a>
          </svg>
        SVG
      end

      it "remaps links before applying suffixes" do
        document.namespace(3, links_map, xpath_to_remove, id_suffix: "DOC_1")

        # Links should be remapped to their targets
        expect(document.content).to match(/xlink:href="#ref1"/)
        expect(document.content).to match(/xlink:href="#external_ref"/)
      end
    end

    context "with xpath removal" do
      let(:input) do
        <<~SVG
          <svg xmlns="http://www.w3.org/2000/svg">
            <?xml-stylesheet type="text/css" href="style.css"?>
            <rect id="rect1" x="0" y="0" width="10" height="10"/>
          </svg>
        SVG
      end
      # Use the same xpath as the actual code (PROCESSING_XPATH)
      let(:xpath_to_remove) { "processing-instruction()|.//processing-instruction()" }

      it "removes elements matching the xpath" do
        document.namespace(0, links_map, xpath_to_remove)

        # Processing instruction should be removed
        expect(document.content).not_to include("<?xml-stylesheet")
        # But the rect should still be there with suffix
        expect(document.content).to match(/id="rect1_000000000"/)
      end
    end

    it "returns self for chaining" do
      result = document.namespace(1, links_map, xpath_to_remove)
      expect(result).to eq(document)
    end
  end
end
