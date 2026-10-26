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
end
