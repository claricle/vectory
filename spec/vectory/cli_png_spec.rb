# frozen_string_literal: true

require "spec_helper"
require "vectory/cli"
require "svgpng"
require "socket"
require "fiddle"

RSpec.describe Vectory::CLI, :png_cli do
  let(:svg) { '<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="red"/></svg>' }
  let(:input) { File.join(directory, "input.svg") }
  let(:output) { File.join(directory, "output.png") }
  let(:native_result) { Svgpng.render(svg, profile: {}, fonts: {}, fallback_fonts: []) }

  let(:directory) { Dir.mktmpdir }

  after { FileUtils.remove_entry(directory) }

  before do
    File.binwrite(input, svg)
    allow(Vectory.ui).to receive(:info)
    allow(Vectory.ui).to receive(:error)
  end

  shared_examples "PNG options and warnings" do
    it "preserves repeated family faces and fallback order at the public render boundary" do
      result = native_result
      first = File.join(directory, "regular.ttf")
      second = File.join(directory, "bold.ttf")
      File.binwrite(first, "regular\x00face".b)
      File.binwrite(second, "bold\x00face".b)
      expect(Svgpng).to receive(:render) do |_content, profile:, fonts:, fallback_fonts:|
        expect(profile).to eq({})
        expect(fonts.keys).to eq(["Family"])
        expect(fonts.fetch("Family").map(&:bytes)).to eq(["regular\x00face".b, "bold\x00face".b])
        expect(fonts.fetch("Family").map(&:family)).to eq(["Family", "Family"])
        expect(fallback_fonts.map(&:source_id)).to eq([second, first])
        expect(fallback_fonts.map(&:family)).to eq(["Second", "First"])
        result
      end
      options = ["--font", "Family=#{first}", "--font", "Family=#{second}",
                 "--fallback-font", "Second=#{second}", "--fallback-font", "First=#{first}"]
      expect(capture_png_cli(png_cli_arguments(input, output, *options)).first).to eq(0)
      expect(File.binread(output)).to eq(result.png_bytes)
    end

    it "prints warnings once in renderer order and leaves PNG bytes untouched" do
      bytes = native_result.png_bytes
      first = png_cli_warning("First", 65)
      second = png_cli_warning("Second", 66)
      duplicate = png_cli_warning("First", 65)
      result = Svgpng::Result.new(png_bytes: bytes, width: 2, height: 1,
                                  warnings: [first, second, duplicate])
      allow(Svgpng).to receive(:render).and_return(result)
      status, stderr = capture_png_cli(png_cli_arguments(input, output))
      expect(status).to eq(0)
      expect(stderr.lines).to eq([
                                   "Font substitution: First -> Fallback; style=normal; weight=400; stretch=normal; scalars=U+0041\n",
                                   "Font substitution: Second -> Fallback; style=normal; weight=400; stretch=normal; scalars=U+0042\n",
                                 ])
      expect(File.binread(output)).to eq(bytes)
    end
  end

  it_behaves_like "PNG options and warnings"

  it "writes a real native-rendered binary PNG with the source dimensions" do
    expect(capture_png_cli(png_cli_arguments(input, output))).to eq([0, ""])
    bytes = File.binread(output)
    expect(bytes).to start_with("\x89PNG\r\n\x1a\n".b)
    expect(bytes.byteslice(16, 8).unpack("NN")).to eq([2, 1])
    expect(File.binread(input)).to eq(svg)
  end

  it "applies a decimal scale through the actual renderer" do
    status, = capture_png_cli(png_cli_arguments(input, output, "--scale", "2"))
    expect(status).to eq(0)
    expect(File.binread(output).byteslice(16, 8).unpack("NN")).to eq([4, 2])
  end

  it "forwards every profile option to the public render boundary" do
    result = native_result
    allow(Svgpng).to receive(:render).with(
      Vectory::Svg.from_content(svg).content,
      profile: { width: 6, height: 3, scale: "2", background: "#12345678" },
      fonts: {}, fallback_fonts: []
    ).and_return(result)
    expect(Svgpng).to receive(:render)
    status, = capture_png_cli(png_cli_arguments(input, output, "--width", "6",
                                                "--height", "3", "--scale", "2",
                                                "--background", "#12345678"))
    expect(status).to eq(0)
    expect(File.binread(output)).to eq(result.png_bytes)
  end

  context "with a delegated EMF source" do
    let(:input) { File.join(directory, "input.emf") }

    before do
      File.binwrite(input, Vectory::FileMagic::EMF_MAGIC)
      source = Vectory::Emf.from_path(input)
      allow(Vectory::Emf).to receive(:from_path).with(input).and_return(source)
      allow(source).to receive(:to_svg).and_return(Vectory::Svg.from_content(svg))
    end

    it_behaves_like "PNG options and warnings"

    it "forwards profile options through the SVG projection" do
      result = native_result
      allow(Svgpng).to receive(:render).with(
        anything, profile: { width: 8 }, fonts: {}, fallback_fonts: []
      ).and_return(result)
      expect(Svgpng).to receive(:render)
      status, = capture_png_cli(png_cli_arguments(input, output, "--width", "8"))
      expect(status).to eq(0)
      expect(File.binread(output)).to eq(result.png_bytes)
    end

    it "maps a renderer subclass failure to the conversion exit status" do
      allow(Svgpng).to receive(:render).and_raise(Svgpng::DuplicateFontError, "duplicate face")
      expect(Vectory.ui).to receive(:error).with("duplicate face")
      expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(4)
      expect(File).not_to exist(output)
    end

    it "maps a renderer system failure to the system exit status" do
      allow(Svgpng).to receive(:render).and_raise(Svgpng::EncodingError, "encoder failed")
      expect(Vectory.ui).to receive(:error).with("encoder failed")
      expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(5)
      expect(File).not_to exist(output)
    end
  end

  [Svgpng::ProfileError, Svgpng::InputError, Svgpng::FontError,
   Svgpng::FontMetadataError, Svgpng::DuplicateFontError,
   Svgpng::MissingFamilyError, Svgpng::MissingGlyphError,
   Svgpng::ForbiddenReferenceError, Svgpng::LimitError].each do |error_class|
    it "maps #{error_class} to conversion exit 4 without a raw exception" do
      allow(Svgpng).to receive(:render).and_raise(error_class, "render rejected")
      expect(Vectory.ui).to receive(:error).with("render rejected")
      expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(4)
      expect(File).not_to exist(output)
    end
  end

  [Svgpng::NativeError, Svgpng::EncodingError, LoadError].each do |error_class|
    it "maps #{error_class} to system exit 5 without a raw exception" do
      allow(Svgpng).to receive(:render).and_raise(error_class, "renderer unavailable")
      expect(Vectory.ui).to receive(:error).with("renderer unavailable")
      expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(5)
      expect(File).not_to exist(output)
    end
  end

  it "maps the unsupported-platform shell failure to system exit 5" do
    error = Svgpng::UnsupportedPlatformError.new(platform: "test-platform", supported_targets: [])
    allow(Svgpng).to receive(:render).and_raise(error)
    expect(Vectory.ui).to receive(:error).with(error.message)
    expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(5)
    expect(File).not_to exist(output)
  end

  [["--width", "zero"], ["--height", "0"], ["--scale", "NaN"],
   ["--background", "blue"], ["--font", "missing-assignment"],
   ["--fallback-font", "=empty-family"]].each do |option|
    it "rejects invalid #{option.join(' ')} as conversion exit 4" do
      expect(capture_png_cli(png_cli_arguments(input, output, *option)).first).to eq(4)
      expect(File).not_to exist(output)
    end
  end

  it "maps an unreadable font file to system exit 5" do
    options = ["--font", "Family=#{File.join(directory, 'missing.ttf')}"]
    expect(Vectory.ui).to receive(:error).with(start_with("Cannot read font:"))
    expect(capture_png_cli(png_cli_arguments(input, output, *options)).first).to eq(5)
    expect(File).not_to exist(output)
  end

  %w[font fallback-font width height scale background].each do |option|
    it "rejects --#{option} for non-PNG output before conversion" do
      expect(Svgpng).not_to receive(:render)
      expect(Vectory.ui).to receive(:error).with(/require PNG output/)
      status, = capture_png_cli(["-f", "emf", "-o", output, "--#{option}", "value", input])
      expect(status).to eq(4)
      expect(File).not_to exist(output)
    end
  end

  it "rejects a DTD without reading a secondary file or opening a socket" do
    File.binwrite(input, "<!DOCTYPE svg [<!ENTITY external SYSTEM \"file:///secret\">]>#{svg}")
    expect(File).to receive(:read).with(input, any_args).at_least(:once).and_call_original
    expect(File).not_to receive(:binread)
    expect(File).not_to receive(:open)
    expect(Socket).not_to receive(:new)
    expect(TCPSocket).not_to receive(:new)
    expect(Leptris::XML).not_to receive(:parse)
    expect(Svgpng).not_to receive(:render)
    expect(Vectory.ui).to receive(:error).with(/DOCTYPE/)
    expect(capture_png_cli(png_cli_arguments(input, output)).first).to eq(4)
    expect(File).not_to exist(output)
  end
end
