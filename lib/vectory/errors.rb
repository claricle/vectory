# frozen_string_literal: true

module Vectory
  class ConversionError < Error; end

  class InkscapeNotFoundError < Error
    def initialize(msg = nil)
      super(msg || "Inkscape not found in PATH. Please install Inkscape.")
    end
  end

  class GhostscriptNotFoundError < Error
    def initialize(msg = nil)
      super(msg || "Ghostscript not found in PATH. Please install Ghostscript.")
    end
  end

  class InkscapeQueryError < Error; end

  class InvalidFormatError < Error
    def initialize(format, supported_formats)
      super("Invalid format '#{format}'. Supported formats: #{supported_formats.join(', ')}")
    end
  end
end
