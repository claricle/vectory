# frozen_string_literal: true

require "spec_helper"

RSpec.describe Vectory::SvgTransform do
  let(:flow) { consumer_flow(transform_fixture("consumer_flow/original.xml")) }

  it "executes the documented migration with inherited namespaces and repeated root IDs" do
    original_host = '<doc xmlns:xlink="http://www.w3.org/1999/xlink" xmlns="http://www.w3.org/2000/svg">' \
                    '<svg id="r"><use xlink:href="#r"/><g xmlns="urn:foreign"><plain xmlns=""/></g></svg>' \
                    '<svg id="r"><use xlink:href="#r"/></svg></doc>'
    reference = File.read("docs/reference/api.adoc")
    example = reference.split("=== Standoc migration boundary").last[/\[source,ruby\]\n----\n(.*?)\n----/m, 1]
    context = binding
    eval(example, context, "docs/reference/api.adoc") # rubocop:disable Security/Eval
    result = Nokogiri::XML(context.local_variable_get(:document).to_xml, &:strict)
    roots = result.xpath("/*/*[local-name()='svg']")
    expect(roots.map { |svg| svg["id"] }).to eq(%w[r r_inject_1])
    expect(roots.map { |svg| svg.namespace.href }).to eq(["http://www.w3.org/2000/svg"] * 2)
    expect(result.xpath("//*[local-name()='use']/@xlink:href", "xlink" => "http://www.w3.org/1999/xlink").map(&:value))
      .to eq(["#r", "#r_inject_1"])
    expect(result.at_xpath("//*[local-name()='g']").namespace.href).to eq("urn:foreign")
    expect(result.at_xpath("//*[local-name()='plain']").namespace).to be_nil
  end

  %w[ISO-8859-1 UTF-16].each do |encoding|
    it "normalizes a #{encoding} graft envelope throughout G/D/S/N without changing its captured input" do
      host = %(<?xml version="1.0" encoding="#{encoding}"?><!--été--><svg id="r" class="café"/><!--fin-->).encode(encoding).b
      graft = described_class.graft_into(host_content: host)
      pipeline = graft >> described_class.disambiguate(index: 1, duplicate_ids: { "r" => true }) >>
        described_class.suffix_classes(index: 1) >> described_class.bind_namespaces
      output = pipeline.call("<svg><text>été 日本語</text></svg>")
      result = Nokogiri::XML(output, &:strict)
      expect(graft.options.fetch(:host_content)).to eq(host)
      expect(output).to start_with('<?xml version="1.0" encoding="UTF-8"?><!--été-->')
      expect(output).to end_with("<!--fin-->")
      expect(result.root["id"]).to eq("r_inject_1")
      expect(result.root["class"]).to eq("café_inject_1")
      expect(result.at_xpath("//text").text).to eq("été 日本語")
      expect(pipeline.call(output)).to eq(output)
    end
  end

  it "pins exactly the public executable profiles and their canonical sequences" do
    expected = Vectory::SvgTransformPipeline::PROFILES.transform_keys(&:to_s)
      .transform_values { |steps| steps.map(&:to_s) }
    expect(transform_manifest.transform_values { |contract| contract.fetch("steps") }).to eq(expected)
    expect(transform_manifest.keys).to contain_exactly("graft", "svg_cleanup", "graft_then_svg_cleanup")
  end

  %w[graft svg_cleanup graft_then_svg_cleanup].each do |name|
    context "with the #{name} profile" do
      let(:contract) { transform_manifest.fetch(name) }
      let(:pipeline) { manifest_pipeline(contract) }
      let(:output) { pipeline.call(transform_fixture(contract.fetch("input"))) }
      let(:document) { Nokogiri::XML(output, &:strict) }

      it "matches its checked-in golden and reparses" do
        expect(pipeline.profile.to_s).to eq(name)
        expect(output).to be_xml_equivalent_to(transform_fixture(contract.fetch("output")))
        expect(document.root).not_to be_nil
      end

      it "is idempotent with the same captured inputs" do
        expect(pipeline.call(output)).to be_xml_equivalent_to(output)
      end

      if name == "graft"
        it "retains captured root metadata while replacing the complete content" do
          expect(document.root.name).to eq("article")
          expect(document.root.namespace.href).to eq("urn:host")
          expect(document.root["id"]).to eq("original")
          expect(document.root["meta:owner"]).to eq("editor")
          expect(document.root["xml:lang"]).to eq("en")
          expect(document.root["discarded"]).to be_nil
          expect(document.at_xpath("//*[local-name()='old']")).to be_nil
          expect(document.xpath("//*[local-name()='svg']").map { |svg| svg["id"] }).to eq(%w[one two])
          expect(document.at_xpath("//*[local-name()='g']")["class"]).to eq("plain")
          expect(document.at_xpath("//comment()").text).to eq("mapped content")
        end
      else
        it "retains rewritten references, class lists and final namespaces" do
          expect(document.root["id"]).to eq("drawing_inject_1")
          expect(document.root["class"]).to eq("drawing_inject_1 active_inject_1")
          expect(document.root.namespaces).to include("xmlns:link" => "http://www.w3.org/1999/xlink")
          expect(document.root.namespaces).not_to have_key("xmlns:xlink")
          expect(document.root.namespaces).not_to have_key("xmlns:unused")
          expect(document.at_xpath("//*[local-name()='linearGradient']")["id"]).to eq("paint_inject_1")
          expect(document.at_xpath("//*[local-name()='g']")["fill"]).to eq("url(#paint_inject_1)")
          expect(document.at_xpath("//*[local-name()='use']")["link:href"]).to eq("#drawing_inject_1")
          expect(document.at_xpath("//*[local-name()='a']")["href"]).to eq("#drawing_inject_1")
          expect(document.at_xpath("//*[local-name()='rect']")["class"]).to eq("active_inject_1")
        end

        it "retains nested selectors and non-selector CSS content" do
          css = document.at_xpath("//*[local-name()='style']").text
          expect(css).to include(".drawing_inject_1.active_inject_1:hover", ".drawing_inject_1:is(.active_inject_1, :not(.hidden_inject_1))")
          expect(css).to include("@supports", "url(#paint_inject_1)", 'content: ".active"', "/* .drawing stays */")
        end

        if name == "graft_then_svg_cleanup"
          it "preserves captured SVG root metadata through later namespace changes" do
            expect(document.root["data-owner"]).to eq("original")
            expect(document.root["viewBox"]).to eq("0 0 40 20")
            expect(document.root["xml:lang"]).to eq("en")
            expect(document.at_xpath("//*[local-name()='old']")).to be_nil
          end
        end
      end
    end
  end

  it "maps the complete host, grafts once, then cleans selected SVGs against its golden" do
    original = transform_fixture("consumer_flow/original.xml")
    expect(Vectory::SvgMapping).to receive(:new).with(original).and_call_original
    expect(Vectory::SvgSubtreeGraft).to receive(:replace_root_content).once.and_call_original
    expect(flow.fetch(:mapped)).to be_xml_equivalent_to(transform_fixture("consumer_flow/mapped.xml"))
    expect(flow.fetch(:output)).to be_xml_equivalent_to(transform_fixture("consumer_flow/expected.xml"))
    expect(flow.fetch(:graft).profile).to eq(:graft)
    expect(flow.fetch(:records).map { |record| record.fetch(:pipeline).profile }).to eq(%i[svg_cleanup svg_cleanup])
  end

  it "grafts a real whole-host svgmap expansion before selecting individual SVGs" do
    host = transform_fixture("consumer_flow/svgmap-original.xml")
    mapped = Vectory::SvgMapping.new(host).to_xml
    expect(mapped).to be_xml_equivalent_to(transform_fixture("consumer_flow/svgmap-mapped.xml"))
    pipeline = described_class.graft_into(host_content: host)
    result = Nokogiri::XML(pipeline.call(mapped), &:strict)
    expect(result.xpath("//svgmap")).to be_empty
    expect(result.root["id"]).to eq("mapping-host")
    expect(result.root["data-owner"]).to eq("original")
    expect(result.root["xml:lang"]).to eq("en")
    expect(result.xpath("/article/figure/*[local-name()='svg']").map { |svg| svg["id"] })
      .to eq(%w[drawing_000000000 drawing_000000001])
    expect(result.at_xpath("//*[local-name()='a']")["href"]).to eq("#chapter")
  end

  it "preserves the first repeated namespaced root and suffixes the later root by position" do
    mapped = Nokogiri::XML(flow.fetch(:mapped), &:strict)
    expect(mapped.xpath("/article/figure/*[local-name()='svg']").map { |svg| svg["id"] })
      .to eq(%w[shared-root shared-root])
    document = Nokogiri::XML(flow.fetch(:output), &:strict)
    roots = document.xpath("/article/figure/*[local-name()='svg']")
    expect(roots.map { |svg| svg["id"] }).to eq(%w[shared-root shared-root_inject_1])
    expect(roots.map { |svg| svg.namespace.href }.uniq).to eq(["http://www.w3.org/2000/svg"])
  end

  it "retains each selected SVG's namespaces, classes and references" do
    document = Nokogiri::XML(flow.fetch(:output), &:strict)
    roots = document.xpath("/article/figure/*[local-name()='svg']")
    roots.each_with_index do |svg, index|
      expect(svg["class"]).to eq("drawing_inject_#{index} active_inject_#{index}")
      expect(svg.namespaces).to include("xmlns:link" => "http://www.w3.org/1999/xlink")
      expect(svg.namespaces).not_to have_key("xmlns:xlink")
      expect(svg.namespaces).not_to have_key("xmlns:unused")
      expect(svg.at_xpath(".//*[local-name()='use']")["link:href"]).to eq("##{svg['id']}")
      expect(svg.at_xpath(".//*[local-name()='a']")["href"]).to eq("##{svg['id']}")
      expect(svg.at_xpath(".//*[local-name()='g']")["fill"]).to eq("url(#paint_inject_#{index})")
    end
  end

  it "retains prepared host metadata and unselected content" do
    document = Nokogiri::XML(flow.fetch(:output), &:strict)
    expect(document.at_xpath("//*[local-name()='rect']")["class"]).to eq("active_inject_0")
    expect(document.root["id"]).to eq("prepared-host")
    expect(document.root["meta:owner"]).to eq("editor")
    expect(document.root["xml:lang"]).to eq("en")
    expect(document.root["data-version"]).to eq("original")
    expect(document.at_xpath("/article/p").text).to eq("Unchanged host content.")
  end

  it "captures a fresh deeply frozen collision snapshot before each SVG" do
    first, second = flow.fetch(:records)
    first_snapshot = first.fetch(:pipeline).steps.first.options.fetch(:duplicate_ids)
    second_snapshot = second.fetch(:pipeline).steps.first.options.fetch(:duplicate_ids)
    expect(first.fetch(:pipeline)).not_to equal(second.fetch(:pipeline))
    expect(first_snapshot).not_to have_key("shared-root")
    expect(second_snapshot).to include("shared-root" => 1, "paint_inject_0" => 1)
    flow.fetch(:records).each do |record|
      snapshot = record.fetch(:pipeline).steps.first.options.fetch(:duplicate_ids)
      expect(record.fetch(:after)).to eq(record.fetch(:before))
      expect(snapshot).to eq(record.fetch(:before))
      expect(snapshot).to be_frozen
      expect(snapshot.fetch("metadata")).to be_frozen
      expect(snapshot.fetch("metadata").first).to be_frozen
    end
  end

  it "keeps captured collision snapshots independent of later caller mutations" do
    first, second = flow.fetch(:records)
    first_snapshot = first.fetch(:pipeline).steps.first.options.fetch(:duplicate_ids)
    second_snapshot = second.fetch(:pipeline).steps.first.options.fetch(:duplicate_ids)
    flow.fetch(:collisions)["shared-root"] = 99
    flow.fetch(:collisions).fetch("metadata") << "later mutation"
    expect(first_snapshot).to eq("paint" => 1, "metadata" => ["caller-owned"])
    expect(second_snapshot.fetch("shared-root")).to eq(1)
    expect(second_snapshot.fetch("metadata")).to eq(["caller-owned"])
    flow.fetch(:records).each do |record|
      expect(record.fetch(:pipeline).call(record.fetch(:input)))
        .to be_xml_equivalent_to(record.fetch(:output))
    end
  end

  it "is idempotent when the complete mapping, graft and sequential cleanup are repeated" do
    repeated = consumer_flow(transform_fixture("consumer_flow/original.xml"), source: flow.fetch(:output))
    expect(repeated.fetch(:output)).to be_xml_equivalent_to(flow.fetch(:output))
  end

  it "pins the separate batch duplicate-ID behavior without using it for consumer cleanup" do
    document = Nokogiri::XML(transform_fixture("consumer_flow/mapped.xml"))
    svgs = document.xpath("/article/figure/*[local-name()='svg']").map(&:to_xml)
    duplicates = Vectory::SvgIdDisambiguator.duplicate_ids(svgs)
    expect(duplicates).to eq("shared-root" => 2, "paint" => 2)
    outputs = svgs.map.with_index do |svg, index|
      Vectory::SvgIdDisambiguator.rewrite(svg, "_inject_#{index}", duplicates)
    end
    expect(outputs.first).to be_xml_equivalent_to(transform_fixture("consumer_flow/batch-first.svg"))
    expect(outputs.last).to be_xml_equivalent_to(transform_fixture("consumer_flow/batch-second.svg"))
    expect(outputs.map { |svg| Nokogiri::XML(svg).root["id"] }).to eq(%w[shared-root_inject_0 shared-root_inject_1])
  end

  context "with a grafted SVG envelope and namespace remap" do
    let(:host) do
      '<?xml version="1.0"?><!-- lead --><!DOCTYPE svg>' \
        '<svg xmlns="http://www.w3.org/2000/svg" width="10" id="host"><title>old</title></svg><!-- trail -->'
    end
    let(:graft) { described_class.graft_into(host_content: host) }
    let(:source) do
      '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">' \
        '<style>.shape { fill: red }</style><defs><linearGradient id="g"/></defs>' \
        '<rect id="r" xlink:href="#g" fill="url(#g)" class="shape"/></svg>'
    end
    let(:pipeline) do
      graft >> described_class.disambiguate(index: 2, duplicate_ids: { "g" => 2, "r" => 2 }) >>
        described_class.suffix_classes(index: 2) >>
        described_class.bind_namespaces(unbind: ["xlink"], remap: { "http://www.w3.org/1999/xlink" => "xl" })
    end

    it "retains href and fill through the public graft step" do
      rect = Nokogiri::XML(graft.call(source), &:strict).at_xpath("//*[local-name()='rect']")
      expect([rect["xlink:href"], rect["fill"]]).to eq(["#g", "url(#g)"])
    end

    it "retains and rewrites href and fill through all four steps" do
      rect = Nokogiri::XML(pipeline.call(source), &:strict).at_xpath("//*[local-name()='rect']")
      expect([rect["xl:href"], rect["fill"]]).to eq(["#g_inject_2", "url(#g_inject_2)"])
    end

    it "retains the captured envelope order after cleanup" do
      output = pipeline.call(source)
      expect(output).to start_with('<?xml version="1.0"?><!-- lead --><!DOCTYPE svg>')
      expect(output).to end_with("<!-- trail -->")
    end

    it "does not add namespace declarations on repeated execution" do
      first = pipeline.call(source)
      repeated = pipeline.call(first)
      expect(repeated).to eq(first)
    end

    it "preserves descendant namespace shadows and default undeclarations on repeat" do
      input = source.sub("</svg>", '<g xmlns="urn:foreign"><g xmlns="urn:foreign"><plain xmlns=""/></g></g></svg>')
      first = pipeline.call(input)
      names = lambda do |xml|
        Nokogiri::XML(xml, &:strict).xpath("//*").map { |element| [element.name, element.namespace&.href] }
      end

      expect(names.call(first)).to eq(names.call(input))
      expect(pipeline.call(first)).to eq(first)
    end
  end
end
