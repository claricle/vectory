# frozen_string_literal: true

module Vectory
  # Keep the optional native dependency behind the PNG projection boundary.
  module PngRenderer
    def self.render(content, profile:, fonts:, fallback_fonts:)
      with_errors do
        result = Svgpng.render(content, profile: profile, fonts: fonts,
                                        fallback_fonts: fallback_fonts)
        Png.new(result.png_bytes, warnings: result.warnings)
      end
    end

    def self.with_errors
      require "svgpng"
      yield
    rescue LoadError => e
      raise RendererLoadError, e.message
    rescue Svgpng::Error => e
      raise mapped_error(e), e.message
    end

    # An ordered list preserves specific subclasses before their parent category.
    def self.mapped_error(error)
      mappings = {
        Svgpng::ProfileError => RenderProfileError,
        Svgpng::FontError => FontError,
        Svgpng::ForbiddenReferenceError => ForbiddenExternalReferenceError,
        Svgpng::LimitError => RenderLimitError,
        Svgpng::InputError => MalformedSvgError,
        Svgpng::UnsupportedPlatformError => UnsupportedPlatformError,
        Svgpng::EncodingError => PngEncodingError,
        Svgpng::NativeError => NativeRenderError,
      }
      mappings.find { |type, _target| error.is_a?(type) }&.last || NativeRenderError
    end
    private_class_method :mapped_error
  end
end
