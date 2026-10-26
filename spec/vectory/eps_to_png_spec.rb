# frozen_string_literal: true

require "spec_helper"
require "svgpng"

RSpec.describe Vectory::Eps, "#to_png" do
  let(:image) { described_class.from_content("source") }
  let(:svg) { Vectory::Svg.from_content('<svg width="2" height="3"/>') }
  let(:profile) { { width: 4 } }
  let(:fonts) { { "Family" => [] } }
  let(:fallbacks) { [] }

  before { allow(image).to receive(:to_svg).and_return(svg) }

  it "delegates unchanged options and returns the PNG with ordered warnings" do
    warnings = %i[first second]
    result = instance_double(Svgpng::Result, png_bytes: "png".b, warnings: warnings)
    allow(Svgpng).to receive(:render).with(svg.content, profile: profile, fonts: fonts,
                                                        fallback_fonts: fallbacks).and_return(result)
    rendered = image.to_png(profile: profile, fonts: fonts, fallback_fonts: fallbacks)
    expect(rendered).to be_a(Vectory::Png)
    expect(rendered.content).to eq("png".b)
    expect(rendered.warnings).to eq(warnings)
  end

  it "propagates mapped render failures with their cause" do
    original = Svgpng::FontError.new("missing face")
    allow(Svgpng).to receive(:render).and_raise(original)
    expect { image.to_png(profile: profile, fonts: fonts, fallback_fonts: fallbacks) }
      .to raise_error(Vectory::FontError) { |error| expect(error.cause).to equal(original) }
  end

  it "retains existing conversion-stage failures" do
    original = Vectory::ConversionError.new("conversion failed")
    allow(image).to receive(:to_svg).and_raise(original)
    expect { image.to_png(fonts: {}, fallback_fonts: []) }.to(raise_error do |error|
      expect(error).to equal(original)
    end)
  end
end
