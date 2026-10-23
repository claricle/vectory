# frozen_string_literal: true

require "yaml"
require "nokogiri"

module SvgTransformHelpers
  def transform_fixture(path)
    File.read(File.join(__dir__, "../fixtures/svg_transform", path))
  end

  def transform_manifest
    YAML.safe_load(transform_fixture("pipeline_profiles.yml"))
  end

  def manifest_pipeline(contract)
    options = contract.fetch("options")
    steps = contract.fetch("steps").map do |name|
      case name
      when "graft_into"
        Vectory::SvgTransform.graft_into(host_content: transform_fixture(contract.fetch("host")))
      when "disambiguate"
        Vectory::SvgTransform.disambiguate(index: options.fetch("index"),
                                         duplicate_ids: options.fetch("duplicate_ids"))
      when "suffix_classes"
        Vectory::SvgTransform.suffix_classes(index: options.fetch("index"))
      when "bind_namespaces"
        Vectory::SvgTransform.bind_namespaces(**options.fetch("namespaces").transform_keys(&:to_sym))
      end
    end
    Vectory::SvgTransformPipeline.new(steps)
  end

  def consumer_namespaces
    {
      bind: { "link" => "http://www.w3.org/1999/xlink" },
      unbind: %w[xlink unused],
      remap: { "http://www.w3.org/1999/xlink" => "link" },
    }
  end

  def consumer_cleanup(index, collisions)
    Vectory::SvgTransform.disambiguate(index: index, duplicate_ids: collisions) >>
      Vectory::SvgTransform.suffix_classes(index: index) >>
      Vectory::SvgTransform.bind_namespaces(**consumer_namespaces)
  end

  # The consumer owns its DOM and evolving collision map. Each public transform
  # crosses the serialized boundary; nested SVGs stay in their outer SVG unit.
  def consumer_flow(original, source: original)
    mapped = Vectory::SvgMapping.new(source).to_xml
    graft = Vectory::SvgTransformPipeline.new([
      Vectory::SvgTransform.graft_into(host_content: original),
    ])
    document = Nokogiri::XML(graft.call(mapped)) { |config| config.strict }
    collisions = { "paint" => 1, "metadata" => ["caller-owned"] }
    records = document.xpath("//*[local-name()='svg' and not(ancestor::*[local-name()='svg'])]")
      .map.with_index do |svg, index|
        pipeline = consumer_cleanup(index, collisions)
        before = Marshal.load(Marshal.dump(collisions))
        input = svg.to_xml
        output = pipeline.call(input)
        after = Marshal.load(Marshal.dump(collisions))
        replacement = Nokogiri::XML(output) { |config| config.strict }.root
        svg.replace(replacement)
        replacement.xpath(". | .//*").each do |element|
          id = element["id"]
          collisions[id] = collisions.fetch(id, 0) + 1 if id
        end
        { pipeline: pipeline, before: before, after: after, input: input, output: output }
      end
    { mapped: mapped, output: document.to_xml, records: records, collisions: collisions, graft: graft }
  end
end

RSpec.configure do |config|
  config.include SvgTransformHelpers
end
