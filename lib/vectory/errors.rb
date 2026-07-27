# frozen_string_literal: true

module Vectory
  class ConversionError < Error; end

  class InvalidFormatError < Error
    def initialize(format, supported_formats)
      super("Invalid format '#{format}'. Supported formats: #{supported_formats.join(', ')}")
    end
  end
end
