# frozen_string_literal: true

require "spec_helper"
require "svgpng"

RSpec.describe Vectory::Png do
  let(:svg) { Vectory::Svg.from_content('<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"/>') }
  let(:png) { svg.to_png(fonts: {}, fallback_fonts: []) }

  it "exposes binary PNG bytes, dimensions and metadata" do
    expect(png.content).to start_with("\x89PNG\r\n\x1a\n".b)
    expect(png.content.encoding).to eq(Encoding::BINARY)
    expect([png.width, png.height]).to eq([2, 3])
    expect(png.mime).to eq("image/png")
    expect(described_class.default_extension).to eq("png")
    expect(png.size).to eq(png.content.bytesize)
    expect(png.warnings).to eq([])
  end

  it "forwards serialized SVG and explicit renderer options without copying warnings" do
    profile = { width: 4 }
    fonts = { "Family" => [] }
    fallbacks = []
    warnings = %i[first second]
    result = instance_double(Svgpng::Result, png_bytes: png.content, warnings: warnings)
    allow(Svgpng).to receive(:render).with(svg.content, profile: profile, fonts: fonts,
                                                        fallback_fonts: fallbacks).and_return(result)

    rendered = svg.to_png(profile: profile, fonts: fonts, fallback_fonts: fallbacks)
    expect(rendered.warnings).to eq(warnings)
    expect(rendered.content).to eq(png.content)
  end

  [described_class, Vectory::Emf].each do |type|
    context "with #{type} shared lifecycle" do
      it "tracks the source separately from the first temporary write and reuses output" do
        Dir.mktmpdir do |directory|
          source = File.join(directory, "source.#{type.default_extension}")
          File.binwrite(source, png.content)
          image = type.from_path(source)
          expect(image.initial_path).to eq(source)
          expect { image.path }.to raise_error(Vectory::NotWrittenToDiskError)
          expect { image.file_size }.to raise_error(Vectory::NotWrittenToDiskError)
          image.write
          expect(image.path).not_to eq(source)
          expect(File.binread(source)).to eq(png.content)
          first_output = image.path
          expect(image.write.path).to eq(first_output)
          expect(image.file_size).to eq(png.size)
          explicit = File.join(directory, "explicit.#{type.default_extension}")
          expect(image.write(explicit).path).to eq(explicit)
          expect(image.write.path).to eq(explicit)
          expect(File.binread(explicit)).to eq(png.content)
        end
      end
    end
  end
end
