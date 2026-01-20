# frozen_string_literal: true

require_relative "../ghostscript_wrapper"
require_relative "strategy"

module Vectory
  module Conversion
    # Ghostscript-based conversion strategy
    #
    # Handles PS/EPS → PDF conversions using Ghostscript.
    # Ghostscript is used for its accurate BoundingBox preservation.
    #
    # @see https://www.ghostscript.com/
    class GhostscriptStrategy < Strategy
      # Ghostscript supports PS/EPS → PDF conversions
      SUPPORTED_CONVERSIONS = [
        %i[ps pdf],
        %i[eps pdf],
      ].freeze

      # Convert PS/EPS content to PDF using Ghostscript
      #
      # @param content [String] the PS/EPS content to convert
      # @param input_format [Symbol] the input format (:ps or :eps)
      # @param output_format [Symbol] the output format (must be :pdf)
      # @param options [Hash] additional options
      # @option options [Boolean] :eps_crop use EPSCrop for better BoundingBox handling
      # @return [String] the PDF content
      # @raise [Vectory::GhostscriptNotFoundError] if Ghostscript is not available
      # @raise [Vectory::ConversionError] if conversion fails
      def convert(content, input_format:, output_format:, **options)
        unless output_format == :pdf
          raise ArgumentError,
                "Ghostscript only supports PDF output, got: #{output_format}"
        end

        unless %i[ps eps].include?(input_format)
          raise ArgumentError,
                "Ghostscript only supports PS/EPS input, got: #{input_format}"
        end

        GhostscriptWrapper.convert(content,
                                   eps_crop: options[:eps_crop] || false)
      end

      # Check if this conversion is supported
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Boolean] true if Ghostscript supports this conversion
      def supports?(input_format, output_format)
        SUPPORTED_CONVERSIONS.include?([input_format, output_format])
      end

      # Get supported conversions
      #
      # @return [Array<Array<Symbol>>] array of [input, output] format pairs
      def supported_conversions
        SUPPORTED_CONVERSIONS
      end

      # Check if Ghostscript is available
      #
      # @return [Boolean] true if Ghostscript can be found
      def available?
        GhostscriptWrapper.available?
      end

      # Get the tool name
      #
      # @return [String] "ghostscript"
      def tool_name
        "ghostscript"
      end
    end
  end
end
