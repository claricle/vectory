# frozen_string_literal: true

require "spec_helper"
require "yaml"

RSpec.describe Vectory::SvgNamespaceBinding do
  let(:fixtures) { File.expand_path("../fixtures/svg_transform/namespace_binding", __dir__) }

  contract = YAML.safe_load_file(File.expand_path("../fixtures/svg_transform/namespace_binding/contract_cases.yml", __dir__))
  phases = %w[parsed reparsed]

  contract.fetch("cases").each do |id, entry|
    context id do
      let(:input) { File.read(File.join(fixtures, "#{id}.svg")) }
      let(:document) { Leptris::XML.parse(input, strict: true) }

      phases.each do |phase|
        it "records #{phase} expanded names and declaration scopes" do
          doc = phase == "parsed" ? document : Leptris::XML.parse(document.to_xml, strict: true)
          snapshot = doc.xpath("//*").map do |node|
            {
              "name" => [node.namespace&.href, node.name.split(":").last],
              "attributes" => node.attribute_nodes.map do |attr|
                [attr.namespace_uri, attr.name.split(":").last, attr.value]
              end,
              "declarations" => node.namespace_definitions.map { |ns| [ns.prefix, ns.href] },
              "in_scope" => node.namespaces,
            }
          end
          expect(snapshot).to eq(entry.fetch("parsed"))
        end
      end

      it "records the measured parser serialization" do
        expect(document.to_xml).to eq(entry.fetch("serialization"))
      end
    end
  end
end
