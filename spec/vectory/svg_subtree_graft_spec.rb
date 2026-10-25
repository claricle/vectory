# frozen_string_literal: true

require "spec_helper"
require "vectory/svg_subtree_graft"

RSpec.describe Vectory::SvgSubtreeGraft do
  let(:host) { transform_fixture("subtree_graft/#{case_name}/host.xml") }
  let(:processed) { transform_fixture("subtree_graft/#{case_name}/processed.xml") }
  let(:result) { described_class.replace_root_content(host, processed) }
  let(:document) { Leptris::XML.parse(result, strict: true) }

  %w[changed metadata mixed namespaces empty repeat].each do |name|
    context "with #{name} content" do
      let(:case_name) { name }

      it "matches the complete expected document" do
        golden = Leptris::XML.parse(transform_fixture("subtree_graft/#{case_name}/expected.xml"), strict: true)
        expect(document.canonicalize(with_comments: true))
          .to eq(golden.canonicalize(with_comments: true))
      end

      it "is XML-equivalent when applied repeatedly" do
        repeated = described_class.replace_root_content(result, processed)
        expect(Leptris::XML.parse(repeated).canonicalize(with_comments: true))
          .to eq(document.canonicalize(with_comments: true))
      end
    end
  end

  context "with host metadata" do
    let(:case_name) { "metadata" }

    it "retains the root attributes and document envelope" do
      expect(document.root.name).to eq("article")
      expect(document.root["id"]).to eq("host")
      expect(document.root["h:flag"]).to eq("yes")
      expect(result).to include(
        "<!--before--><?host keep?><!DOCTYPE article>",
        "</article><!--after--><?tail keep?>",
      )
      expect(result).not_to include('id="discard"', "<old")
    end
  end

  context "with mixed content" do
    let(:case_name) { "mixed" }

    it "preserves the ordered node kinds and values" do
      children = document.root.children
      expect(children.map { |node| node.class.name.split("::").last })
        .to eq(%w[Text Comment CDATA ProcessingInstruction Element Text])
      expect(result).to include("before<!--middle--><![CDATA[a<b & c]]><?graft keep?>")
      expect(children.last.text).to eq("after")
    end
  end

  it "retains an envelope containing markup-like text and whitespace" do
    prefix = "<?xml version=\"1.0\"?>\n<!--é--><!DOCTYPE host [<!ELEMENT host ANY>]>\n"
    suffix = " \n<!-- </host> --><?tail </host>?>\n"
    content = "#{prefix}<host><!--old--><old/></host>#{suffix}"
    output = described_class.replace_root_content(content, "<svg/>")

    expect(output).to eq("#{prefix}<host/>#{suffix}")
  end

  context "with conflicting namespaces" do
    let(:case_name) { "namespaces" }

    it "retains host metadata and grafted expanded names after reparsing" do
      expect(document.root.name).to eq("host")
      expect(document.root.namespaces).to eq(Leptris::XML.parse(host).root.namespaces)
      elements = document.xpath("//*")
      expect(elements.map { |node| [node.name, node.namespace&.href] })
        .to eq([
                 ["host", "urn:host"], ["g", "urn:svg"],
                 ["shape", "urn:processed-prefix"], ["plain", nil],
                 ["child", nil], ["other", "urn:processed-prefix"], ["bare", nil]
               ])
      expect(elements[1].attribute_nodes.first.namespace_uri).to eq("urn:processed-prefix")
      expect(elements[2].attribute_nodes.first.namespace_uri)
        .to eq("http://www.w3.org/1999/xlink")
    end
  end

  nesting = [false, true]
  [
    'a="1" xlink:href="#g" c="3"',
    'xlink:href="#g" fill="x"',
    'fill="x" xlink:href="#g"',
  ].each do |attributes|
    nesting.each do |nested|
      it "retains every attribute in #{attributes} with nested=#{nested}" do
        rect = "<rect #{attributes}/>"
        content = nested ? "<g>#{rect}</g>" : rect
        source = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">' \
                 "#{content}</svg>"
        output = described_class.replace_root_content('<svg xmlns="http://www.w3.org/2000/svg"/>', source)
        expected = Nokogiri::XML(source, &:strict).at_xpath("//*[local-name()='rect']")
        actual = Nokogiri::XML(output, &:strict).at_xpath("//*[local-name()='rect']")

        expect(actual.attribute_nodes.map { |attr| [attr.namespace&.href, attr.name, attr.value] })
          .to eq(expected.attribute_nodes.map { |attr| [attr.namespace&.href, attr.name, attr.value] })
      end
    end
  end

  context "with invalid input" do
    it "rejects malformed host XML with the stable prefix" do
      expect { described_class.replace_root_content("<host>", "<svg/>") }
        .to raise_error(Vectory::ParsingError, /\Ainvalid host_content XML:/)
    end

    it "rejects malformed processed XML with the stable prefix" do
      expect { described_class.replace_root_content("<host/>", "<svg>") }
        .to raise_error(Vectory::ParsingError, /\Ainvalid processed_content XML:/)
    end

    it "rejects a rootless host with the exact message" do
      expect { described_class.replace_root_content("<!--host-->", "<svg/>") }
        .to raise_error(Vectory::ParsingError, "host_content has no document root")
    end

    it "rejects rootless processed content with the exact message" do
      expect { described_class.replace_root_content("<host/>", "<?xml version='1.0'?>") }
        .to raise_error(Vectory::ParsingError, "processed_content has no document root")
    end

    it "does not classify malformed rootless content as a missing root" do
      expect { described_class.replace_root_content("<!--unterminated", "<svg/>") }
        .to raise_error(Vectory::ParsingError, /\Ainvalid host_content XML:/)
    end
  end

  %w[ISO-8859-1 UTF-16].each do |encoding|
    context "with a #{encoding} host" do
      let(:host) do
        %(<?xml version="1.0" encoding="#{encoding}"?><!--café--><host label="été"/><!--fin été-->).encode(encoding).b
      end
      let(:processed) { "<svg><text>été 日本語</text><!--日本語--><![CDATA[日本語]]></svg>" }

      it "normalizes the encoding while preserving host metadata and the ordered Unicode content" do
        parsed = Nokogiri::XML(result, &:strict)
        expect(result).to start_with('<?xml version="1.0" encoding="UTF-8"?><!--café-->')
        expect(result).to end_with("<!--fin été-->")
        expect(parsed.root["label"]).to eq("été")
        expect(parsed.root.children.map(&:content)).to eq(["été 日本語", "日本語", "日本語"])
        expect(parsed.root.children.map(&:class)).to eq([Nokogiri::XML::Element, Nokogiri::XML::Comment, Nokogiri::XML::CDATA])
      end

      it "remains stable when the normalized output is grafted again" do
        expect(described_class.replace_root_content(result, processed)).to eq(result)
      end
    end
  end

  it "decodes processed content independently of the host encoding" do
    source = '<?xml version="1.0" encoding="ISO-8859-1"?><svg><text>été</text></svg>'.encode("ISO-8859-1").b
    output = described_class.replace_root_content('<host label="日本語"/>', source)
    parsed = Nokogiri::XML(output, &:strict)
    expect(parsed.root["label"]).to eq("日本語")
    expect(parsed.at_xpath("//text").text).to eq("été")
  end

  it "does not mistake an xml-stylesheet processing instruction for the XML declaration" do
    prefix = '<?xml-stylesheet type="text/css" href="café.css" encoding="ISO-8859-1"?>'
    output = described_class.replace_root_content("#{prefix}<host label='été'/>", "<svg/>")
    expect(output).to start_with(prefix)
    expect(Nokogiri::XML(output, &:strict).root["label"]).to eq("été")
  end
end
