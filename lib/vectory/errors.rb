# frozen_string_literal: true

module Vectory
  class ConversionError < Error; end

  class RenderProfileError < ConversionError; end
  class MalformedSvgError < ConversionError; end
  class FontError < ConversionError; end
  class ForbiddenExternalReferenceError < ConversionError; end
  class RenderLimitError < ConversionError; end

  class UnsupportedPlatformError < SystemCallError; end
  class RendererLoadError < SystemCallError; end
  class NativeRenderError < SystemCallError; end
  class PngEncodingError < SystemCallError; end

  class InvalidFormatError < Error
    def initialize(format, supported_formats)
      super("Invalid format '#{format}'. Supported formats: #{supported_formats.join(', ')}")
    end
  end
end
