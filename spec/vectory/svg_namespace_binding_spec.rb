# frozen_string_literal: true

require "spec_helper"
require "yaml"
require "vectory/svg_namespace_binding"

RSpec.describe Vectory::SvgNamespaceBinding do
  fixtures = File.expand_path("../fixtures/svg_transform/namespace_binding", __dir__)
  contract = YAML.safe_load_file(File.join(fixtures, "contract_cases.yml"))

  contract.fetch("cases").each do |id, entry|
    context id do
      let(:input) { File.read(File.join(fixtures, "#{id}.svg")) }
      let(:options) { entry.fetch("operation").transform_keys(&:to_sym) }

      if entry["error"]
        it "rejects atomically with the selected error contract" do
          original = input.dup
          expect { described_class.rewrite(input, **options) }
            .to raise_error(ArgumentError, /#{entry.fetch("error").fetch("pattern")}/)
          expect(input).to eq(original)
        end
      else
        let(:output) { described_class.rewrite(input, **options) }
        let(:repeated) { described_class.rewrite(output, **options) }

        it "matches the first and repeat namespace goldens" do
          expect(output).to be_xml_equivalent_to(File.read(File.join(fixtures, "#{id}.expected.svg")))
          expect(repeated).to be_xml_equivalent_to(File.read(File.join(fixtures, "#{id}.repeat.svg")))
          { "expected" => output, "repeat" => repeated }.each do |suffix, serialized|
            expected = Leptris::XML.parse(File.read(File.join(fixtures, "#{id}.#{suffix}.svg")), strict: true)
            actual = Leptris::XML.parse(serialized, strict: true)
            expect(actual.xpath("//*").map(&:namespaces)).to eq(expected.xpath("//*").map(&:namespaces))
            expect(actual.xpath("//*").map { |node| [node.namespace&.prefix, node.attribute_nodes.map(&:name)] })
              .to eq(expected.xpath("//*").map { |node| [node.namespace&.prefix, node.attribute_nodes.map(&:name)] })
          end
        end

        it "preserves expanded names and attribute values through both applications" do
          [output, repeated].each do |serialized|
            names = Leptris::XML.parse(serialized, strict: true).xpath("//*").map do |node|
              {
                "name" => [node.namespace&.href, node.name.split(":").last],
                "attributes" => node.attribute_nodes.map do |attr|
                  [attr.namespace_uri, attr.name.split(":").last, attr.value]
                end.sort_by(&:inspect),
              }
            end
            original = entry.fetch("parsed").map do |node|
              node.slice("name", "attributes").merge("attributes" => node.fetch("attributes").sort_by(&:inspect))
            end
            expect(names).to eq(original)
          end
        end

        it "does not accumulate namespace declarations on repeat" do
          first = Leptris::XML.parse(output, strict: true)
          second = Leptris::XML.parse(repeated, strict: true)
          expect(second.xpath("//*").map(&:namespaces)).to eq(first.xpath("//*").map(&:namespaces))
          expect(second.xpath("//*").map { |node| node.namespace_definitions.map { |ns| [ns.prefix, ns.href] } })
            .to eq(first.xpath("//*").map { |node| node.namespace_definitions.map { |ns| [ns.prefix, ns.href] } })
        end
      end
    end
  end

  it "preserves comments, processing instructions, CDATA and text spelling" do
    source = File.read(File.join(fixtures, "default_svg.svg"))
    output = described_class.rewrite(source, bind: { "extra" => "urn:extra" })
    expect(output).to include('<!--keep--><?probe keep?><text><![CDATA[a < b]]></text>')
  end

  ["", "<svg>", "<host/>", "<svg/><svg/>", "<svg><p:g/></svg>", '<svg p:a="b"/>', nil].each do |invalid|
    it "rejects invalid SVG input #{invalid.inspect}" do
      expect { described_class.rewrite(invalid) }.to raise_error(ArgumentError, /invalid SVG:/)
    end
  end

  [{ bind: [] }, { unbind: {} }, { remap: [] }, { bind: { "a:b" => "urn:a" } },
   { bind: { "xml" => "urn:a" } }, { bind: { "p" => "" } },
   { bind: { "p" => "http://www.w3.org/XML/1998/namespace" } },
   { bind: { "p" => "http://www.w3.org/2000/xmlns/" } }].each do |options|
    it "rejects invalid options #{options.inspect}" do
      expect { described_class.rewrite("<svg/>", **options) }
        .to raise_error(ArgumentError, /invalid namespace request:/)
    end
  end

  it "rejects a default binding that would change element expanded names" do
    expect { described_class.rewrite('<svg id="plain"/>', bind: { nil => "urn:new" }) }
      .to raise_error(ArgumentError, /default namespace:/)
  end

  it "rejects conflicting requests before rewriting" do
    expect do
      described_class.rewrite("<svg/>", bind: { "p" => "urn:a" }, remap: { "urn:b" => "p" })
    end.to raise_error(ArgumentError, /namespace conflict:/)
  end

  it "rejects conflicting normalized default bindings" do
    expect { described_class.rewrite("<svg/>", bind: { nil => "urn:a", "" => "urn:b" }) }
      .to raise_error(ArgumentError, /namespace conflict:/)
  end

  it "rejects default namespace capture below an explicit undeclaration" do
    source = '<p:svg xmlns:p="urn:svg"><g xmlns=""/></p:svg>'
    expect { described_class.rewrite(source, bind: { nil => "urn:svg" }) }
      .to raise_error(ArgumentError, /namespace conflict:/)
  end

  it "rejects a target also requested for removal" do
    expect { described_class.rewrite("<svg/>", bind: { "p" => "urn:a" }, unbind: ["p"]) }
      .to raise_error(ArgumentError, /namespace conflict:/)
  end
end
