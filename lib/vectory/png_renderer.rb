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
      raise mapped_error(e), describe(e)
    end

    # The native layer reports only a status number; its typed details say why.
    def self.describe(error)
      return error.message unless error.message.match?(/\(status \d+\)\z/)

      explain = status_messages.find { |type, _message| error.is_a?(type) }&.last
      explain ? explain.call(error) : error.message
    end

    def self.status_messages
      {
        Svgpng::FontMetadataError => lambda { |_error|
          "Font face is unreadable, or its own family name differs from the family it was given"
        },
        Svgpng::DuplicateFontError => ->(_error) { "Two font faces share one family, style, weight and stretch" },
        Svgpng::MissingFamilyError => ->(error) { "No font face was given for #{family(error)}" },
        Svgpng::MissingGlyphError => lambda { |error|
          "#{family(error).sub(/\A./, &:upcase)} has no glyph for #{scalars(error)}"
        },
        Svgpng::ForbiddenReferenceError => ->(error) { "External reference forbidden: #{reference(error)}" },
      }
    end

    def self.family(error)
      error.requested_family ? "font family #{error.requested_family.inspect}" : "a font family in the SVG"
    end

    def self.scalars(error)
      values = Array(error.scalars).map { |scalar| format("U+%04X", scalar) }
      values.empty? ? "some characters" : values.join(", ")
    end

    def self.reference(error)
      [error.reference_class, error.location].compact.join(" at ").then { |text| text.empty? ? "unknown" : text }
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
    private_class_method :mapped_error, :describe, :status_messages, :family, :scalars, :reference
  end
end
