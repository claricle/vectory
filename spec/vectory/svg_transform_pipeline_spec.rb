# frozen_string_literal: true

require "spec_helper"

RSpec.describe Vectory::SvgTransformPipeline do
  let(:collisions) { { "shape" => 2, "metadata" => ["original"] } }
  let(:disambiguate) { Vectory::SvgTransform.disambiguate(index: 1, duplicate_ids: collisions) }
  let(:namespaces) { Vectory::SvgTransform.bind_namespaces(bind: { "xlink" => "http://www.w3.org/1999/xlink" }) }
  let(:cleanup) { disambiguate >> Vectory::SvgTransform.suffix_classes(index: 1) >> namespaces }
  let(:svg) do
    '<svg xmlns="http://www.w3.org/2000/svg" id="shape"><style>.label { fill: red }</style><g class="label"/></svg>'
  end

  it "exposes only the three executable profiles" do
    expect(described_class::PROFILES.keys).to eq(%i[graft svg_cleanup graft_then_svg_cleanup])
    expect(cleanup.profile).to eq(:svg_cleanup)
    expect(cleanup.steps.map(&:name)).to eq(%i[disambiguate suffix_classes bind_namespaces])
  end

  it "copies and deeply freezes the collision map at construction" do
    snapshot = disambiguate.options.fetch(:duplicate_ids)
    collisions["shape"] = 0
    collisions["metadata"] << "later"
    expect(snapshot).to eq("shape" => 2, "metadata" => ["original"])
    expect(snapshot).to be_frozen
    expect(snapshot.fetch("metadata")).to be_frozen
    expect(snapshot.fetch("metadata").first).to be_frozen
    expect { snapshot["new"] = 1 }.to raise_error(FrozenError)
  end

  it "builds new values without extending the original prefix" do
    prefix = disambiguate >> Vectory::SvgTransform.suffix_classes(index: 1)
    pipeline = prefix >> namespaces
    expect(prefix.steps.size).to eq(2)
    expect(pipeline.steps.size).to eq(3)
    expect(pipeline).to be_frozen
    expect(pipeline.steps).to be_frozen
  end

  it "rejects incomplete prefixes before parsing input" do
    expect { disambiguate.call("broken XML") }.to raise_error(ArgumentError, /incomplete profile/)
    expect { (disambiguate >> Vectory::SvgTransform.suffix_classes(index: 1)).call("broken XML") }
      .to raise_error(ArgumentError, /incomplete profile/)
  end

  it "rejects skipped, duplicate, reversed and non-leading graft steps on append" do
    graft = Vectory::SvgTransform.graft_into(host_content: "<svg/>")
    expect { disambiguate >> namespaces }.to raise_error(ArgumentError, /invalid step sequence/)
    expect { disambiguate >> disambiguate }.to raise_error(ArgumentError, /invalid step sequence/)
    expect { Vectory::SvgTransform.suffix_classes(index: 1) >> disambiguate }
      .to raise_error(ArgumentError, /invalid step sequence/)
    expect { cleanup >> graft }.to raise_error(ArgumentError, /invalid step sequence/)
  end

  it "rejects complete host input for a per-SVG cleanup" do
    expect { cleanup.call("<host><svg/><svg/></host>") }
      .to raise_error(ArgumentError, /execution-scope mismatch/)
  end

  it "rejects all-step execution with a complete host capture" do
    pipeline = Vectory::SvgTransform.graft_into(host_content: "<host><svg/></host>") >> cleanup
    expect { pipeline.call(svg) }.to raise_error(ArgumentError, /requires an SVG host root/)
  end

  it "uses the same positional suffix for IDs and classes without changing caller state" do
    original = Marshal.load(Marshal.dump(collisions))
    result = Leptris::XML.parse(cleanup.call(svg))
    expect(result.root["id"]).to eq("shape_inject_1")
    expect(result.at_xpath("//*[local-name()='g']")["class"]).to eq("label_inject_1")
    expect(result.at_xpath("//*[local-name()='style']").text).to include(".label_inject_1")
    expect(collisions).to eq(original)
  end

  it "captures host text independently and preserves host metadata through all steps" do
    host = +'<svg xmlns="http://www.w3.org/2000/svg" data-host="retained"><old/></svg>'
    graft = Vectory::SvgTransform.graft_into(host_content: host)
    host.replace("<changed/>")
    pipeline = graft >> cleanup
    result = Leptris::XML.parse(pipeline.call(svg))
    expect(pipeline.profile).to eq(:graft_then_svg_cleanup)
    expect(result.root["data-host"]).to eq("retained")
    expect(result.at_xpath("//*[local-name()='old']")).to be_nil
    expect(result.at_xpath("//*[local-name()='g']")["class"]).to eq("label_inject_1")
    expect(result.root.namespaces).to include("xmlns:xlink" => "http://www.w3.org/1999/xlink")
  end

  it "allows a whole-document graft without per-SVG operations" do
    graft = Vectory::SvgTransform.graft_into(host_content: '<host retained="yes"><old/></host>')
    result = Leptris::XML.parse(graft.call("<mapped><svg/><svg/></mapped>"))
    expect(result.root.name).to eq("host")
    expect(result.root["retained"]).to eq("yes")
    expect(result.xpath("//svg").size).to eq(2)
  end

  it "rejects malformed captured hosts when creating a step" do
    expect { Vectory::SvgTransform.graft_into(host_content: "<svg>") }
      .to raise_error(Vectory::ParsingError, /invalid host_content XML/)
  end

  it "propagates malformed selected input before processing steps" do
    expect { cleanup.call("<svg>") }.to raise_error(Vectory::ParsingError, /invalid content XML/)
  end
end
