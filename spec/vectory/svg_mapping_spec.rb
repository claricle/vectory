require "spec_helper"

RSpec.describe Vectory::SvgMapping do
  context "no namespace" do
    let(:source) { described_class.from_path("doc.xml") }
    let(:reference) { File.read("doc-ref.xml") }
    let(:work_dir) { "spec/examples/svg" }

    it "rewrites ids" do
      Dir.chdir(work_dir) do
        content = source.to_xml
        result = strip_image_and_style(content)

        expect(result).to be_xml_equivalent_to(reference)
      end
    end
  end

  context "with namespaces" do
    # NOTE: Reference file (doc2-ref.xml) is generated using Nokogiri and external SVG files.
    # If this test fails after updating Nokogiri or SVG files, regenerate fixtures with:
    #   bundle exec rake regenerate_fixtures
    let(:source) { described_class.from_path("doc2.xml") }
    let(:reference) { File.read("doc2-ref.xml") }
    let(:work_dir) { "spec/examples/svg" }

    it "rewrites ids" do
      Dir.chdir(work_dir) do
        content = source.to_xml

        result = strip_image(content)

        expect(result).to be_xml_equivalent_to(reference)
      end
    end
  end

  context "with non-existent path of image" do
    let(:source) { described_class.from_path("doc3.xml") }
    let(:reference) { File.read("doc3-ref.xml") }
    let(:work_dir) { "spec/examples/svg" }

    it "processes without failing" do
      Dir.chdir(work_dir) do
        content = source.to_xml

        result = strip_image(content)

        expect(result).to be_xml_equivalent_to(reference)
      end
    end
  end

  def strip_image(xml)
    # Handle both self-closing and regular image tags with content
    # Also handle base64-encoded embedded images
    xml.gsub(%r{<image\b[^>]*>.*?</image>}m, "<image/>")
      .gsub(%r{<image\b[^>]*/>}, "<image/>")
  end

  def strip_image_and_style(xml)
    xml.gsub(%r{<image\b[^>]*>.*?</image>}m, "<image/>")
      .gsub(%r{<image\b[^>]*/>}, "<image/>")
      .gsub(%r{<style\b[^>]*>.*?</style>}m, "<style/>")
  end
end
