# frozen_string_literal: true

require "spec_helper"
require "svgpng"

RSpec.describe Vectory::PngRenderer do
  let(:svg) { Vectory::Svg.from_content('<svg width="2" height="3"/>') }

  {
    Svgpng::ProfileError => Vectory::RenderProfileError,
    Svgpng::InputError => Vectory::MalformedSvgError,
    Svgpng::FontError => Vectory::FontError,
    Svgpng::FontMetadataError => Vectory::FontError,
    Svgpng::DuplicateFontError => Vectory::FontError,
    Svgpng::MissingFamilyError => Vectory::FontError,
    Svgpng::MissingGlyphError => Vectory::FontError,
    Svgpng::ForbiddenReferenceError => Vectory::ForbiddenExternalReferenceError,
    Svgpng::LimitError => Vectory::RenderLimitError,
    Svgpng::NativeError => Vectory::NativeRenderError,
    Svgpng::EncodingError => Vectory::PngEncodingError,
    Svgpng::Error => Vectory::NativeRenderError,
  }.each do |source, target|
    it "maps #{source} to #{target} and retains its cause" do
      original = source.new("renderer failure")
      allow(Svgpng).to receive(:render).and_raise(original)
      expect { svg.to_png(fonts: {}, fallback_fonts: []) }.to raise_error(target) { |error|
        expect(error.cause).to equal(original)
      }
    end
  end

  it "maps unsupported packages and retains platform diagnostics as cause" do
    original = Svgpng::UnsupportedPlatformError.new(platform: "unknown", supported_targets: [])
    allow(Svgpng).to receive(:render).and_raise(original)
    expect { svg.to_png(fonts: {}, fallback_fonts: []) }
      .to raise_error(Vectory::UnsupportedPlatformError) { |error| expect(error.cause).to equal(original) }
  end

  it "maps loader failures without exposing LoadError" do
    original = LoadError.new("packaged renderer unavailable")
    allow(described_class).to receive(:require).with("svgpng").and_raise(original)
    expect { svg.to_png(fonts: {}, fallback_fonts: []) }
      .to raise_error(Vectory::RendererLoadError) { |error| expect(error.cause).to equal(original) }
  end

  describe "messages for bare native status failures" do
    {
      Svgpng::FontMetadataError.new("SVGPNG rendering failed (status 3)") =>
        "Font face is unreadable, or its own family name differs from the family it was given",
      Svgpng::DuplicateFontError.new("SVGPNG rendering failed (status 4)") =>
        "Two font faces share one family, style, weight and stretch",
      Svgpng::MissingFamilyError.new("SVGPNG rendering failed (status 5)", requested_family: "Arial") =>
        'No font face was given for font family "Arial"',
      Svgpng::MissingFamilyError.new("SVGPNG rendering failed (status 5)") =>
        "No font face was given for a font family in the SVG",
      Svgpng::MissingGlyphError.new("SVGPNG rendering failed (status 6)",
                                    requested_family: "Arial", scalars: [0x6F22, 0xE9]) =>
        'Font family "Arial" has no glyph for U+6F22, U+00E9',
      Svgpng::ForbiddenReferenceError.new("SVGPNG rendering failed (status 7)",
                                          reference_class: "file", location: "image@href") =>
        "External reference forbidden: file at image@href",
      Svgpng::ForbiddenReferenceError.new("SVGPNG rendering failed (status 7)") =>
        "External reference forbidden: unknown",
    }.each do |original, message|
      it "explains #{original.class} #{message.inspect}" do
        allow(Svgpng).to receive(:render).and_raise(original)
        expect { svg.to_png(fonts: {}, fallback_fonts: []) }.to raise_error(Vectory::Error, message)
      end
    end

    it "keeps a descriptive native message unchanged" do
      allow(Svgpng).to receive(:render).and_raise(Svgpng::ProfileError, "width exceeds 32768")
      expect { svg.to_png(fonts: {}, fallback_fonts: []) }
        .to raise_error(Vectory::RenderProfileError, "width exceeds 32768")
    end

    it "names the missing family the native renderer reports" do
      svg = Vectory::Svg.from_content('<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">' \
                                      '<text font-family="Probe">x</text></svg>')
      expect { svg.to_png(fonts: {}, fallback_fonts: []) }
        .to raise_error(Vectory::FontError, 'No font face was given for font family "Probe"')
    end
  end
end
