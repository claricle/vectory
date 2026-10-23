# frozen_string_literal: true

require "spec_helper"

RSpec.describe Vectory::SvgTransform do
  let(:flow) { consumer_flow(transform_fixture("consumer_flow/original.xml")) }

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
end
