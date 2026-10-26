# frozen_string_literal: true

require "spec_helper"
require "png_conform"
require "png_conform/services/validation_orchestrator"

# This integration boundary lives beside the validator rather than the PNG unit specs.
RSpec.describe Vectory::Png do # rubocop:disable RSpec/SpecFilePathFormat
  include PngConformHelpers

  let(:svg) do
    Vectory::Svg.new(<<~SVG)
      <svg xmlns="http://www.w3.org/2000/svg" width="48" height="32" viewBox="0 0 48 32">
        <defs><linearGradient id="paint"><stop stop-color="#2468ac"/>
          <stop offset="1" stop-color="#ef8420"/></linearGradient></defs>
        <rect x="2" y="2" width="44" height="28" rx="4" fill="url(#paint)"/>
        <circle cx="24" cy="16" r="9" fill="#ffffff" fill-opacity="0.5"/>
      </svg>
    SVG
  end
  let(:png) { svg.to_png(profile: { scale: "2" }, fonts: {}, fallback_fonts: []) }

  let(:directory) { Dir.mktmpdir("vectory-conformance") }

  after { FileUtils.remove_entry(directory) }

  it "validates the written Vectory PNG through the real file boundary" do
    expect(png).to be_a(described_class)
    path = File.join(directory, "rendered.png")
    png.write(path)

    analysis = PngConform::Services::ValidationOrchestrator.validate_file(path)

    expect(analysis).to be_valid, png_conform_diagnostics(analysis)
    expect([png.width, png.height]).to eq([96, 64])
    expect(File.binread(path)).to eq(png.content)
  end

  it "reports diagnostic errors for a corrupted copy" do
    path = File.join(directory, "rendered.png")
    png.write(path)
    corrupted = File.binread(path)
    corrupted.setbyte(0, 0)
    corrupt_path = File.join(directory, "corrupted.png")
    File.binwrite(corrupt_path, corrupted)

    analysis = PngConform::Services::ValidationOrchestrator.validate_file(corrupt_path)

    expect(analysis).not_to be_valid, png_conform_diagnostics(analysis)
    expect(analysis.errors).not_to be_empty
    expect(File.binread(path)).to eq(png.content)
  end
end
