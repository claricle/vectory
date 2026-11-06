# frozen_string_literal: true

module PngConformHelpers
  def png_conform_diagnostics(analysis)
    "PNG validation failed: #{analysis.errors.inspect}; #{analysis.to_json}"
  end
end
