require "spec_helper"

require "vectory/svg_id_disambiguator"

RSpec.describe Vectory::SvgIdDisambiguator do
  let(:svg_a) do
    <<~SVG
      <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <defs>
          <linearGradient id="grad"><stop stop-color="red"/></linearGradient>
          <clipPath id="clip"><rect width="1" height="1"/></clipPath>
        </defs>
        <style>#grad { fill: url(#grad) }</style>
        <use xlink:href="#grad" href="#grad"/>
        <g id="shape"><path clip-path="url(#clip)" style="fill: url(#grad)"/></g>
      </svg>
    SVG
  end

  let(:svg_b) do
    <<~SVG
      <svg xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="grad"><stop stop-color="blue"/></linearGradient>
          <clipPath id="clip"><rect width="2" height="2"/></clipPath>
        </defs>
        <g id="shape"><circle id="dot" r="1"/></g>
        <use xlink:href="#grad"/>
        <path clip-path="url(#clip)"/>
        <path marker-mid="url(#marker-only)"/>
      </svg>
    SVG
  end

  let(:dupes) { described_class.duplicate_ids([svg_a, svg_b]) }

  describe ".duplicate_ids" do
    it "counts ids that occur in more than one document" do
      expect(dupes).to eq("shape" => 2, "grad" => 2, "clip" => 2)
    end

    it "ignores single-occurrence ids" do
      expect(dupes).not_to include("dot", "marker-only")
    end
  end

  describe ".rewrite" do
    let(:out_a) { described_class.rewrite(svg_a, "_inject_0", dupes) }
    let(:out_b) { described_class.rewrite(svg_b, "_inject_1", dupes) }

    it "suffixes the colliding id attributes" do
      expect(out_a).to include('id="shape_inject_0"')
      expect(out_b).to include('id="shape_inject_1"')
    end

    it "leaves unique ids untouched" do
      expect(out_a).to include('id="grad_inject_0"')
      expect(out_a).to include('id="clip_inject_0"')
      expect(out_b).to include('id="dot"')
    end

    it "rewrites href and xlink:href fragment references" do
      expect(out_a).to include('xlink:href="#grad_inject_0"')
      expect(out_a).to include('href="#grad_inject_0"')
    end

    it "rewrites url() references in style blocks" do
      expect(out_a).to include("fill: url(#grad_inject_0)")
    end

    it "rewrites url() references in presentation attributes" do
      expect(out_a).to include('clip-path="url(#clip_inject_0)"')
      expect(out_a).to include("fill: url(#grad_inject_0)")
    end

    it "keeps non-colliding references untouched" do
      expect(out_b).to include('marker-mid="url(#marker-only)"')
    end


    it "keeps the document parseable and all elements present" do
      doc = Leptris::XML.parse(out_a)
      expect(doc.xpath("//*").map(&:name)).to include("linearGradient", "clipPath", "use", "path")
    end
  end

  describe "host-document form" do
    let(:host) do
      <<~XML
        <metanorma>
          #{svg_a}
          #{svg_b}
        </metanorma>
      XML
    end

    it "rewrites svgs embedded in a host document" do
      dupes = described_class.duplicate_ids([svg_a, svg_b])
      out = described_class.rewrite(host, "_inject_2", dupes)
      expect(out).to include('id="shape_inject_2"')
      expect(out.scan('id="dot"').count).to eq(1)
    end
  end
end
