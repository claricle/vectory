# frozen_string_literal: true

require_relative "conversion/strategy"
require_relative "conversion/inkscape_strategy"
require_relative "conversion/ghostscript_strategy"

module Vectory
  # Conversion module provides strategy-based conversion interface
  #
  # This module encapsulates different conversion strategies for converting
  # between vector formats using external tools like Inkscape and Ghostscript.
  #
  # @example Convert SVG to EPS using Inkscape
  #   Vectory::Conversion.convert(svg_content, from: :svg, to: :eps)
  #
  # @example Get available strategies for a conversion
  #   Vectory::Conversion.strategies_for(:svg, :eps)
  module Conversion
    class << self
      # Convert content from one format to another
      #
      # Automatically selects the appropriate strategy based on the input/output formats.
      #
      # @param content [String] the content to convert
      # @param from [Symbol] the input format
      # @param to [Symbol] the output format
      # @param options [Hash] additional options passed to the strategy
      # @return [Vectory::Vector, String] the converted result
      # @raise [Vectory::ConversionError] if no strategy supports the conversion
      def convert(content, from:, to:, **options)
        strategy = find_strategy(from, to)

        unless strategy
          supported = supported_conversions.map do |a, b|
            "#{a} → #{b}"
          end.join(", ")
          raise Vectory::ConversionError,
                "No strategy found for #{from} → #{to} conversion. " \
                "Supported: #{supported}"
        end

        strategy.convert(content, input_format: from, output_format: to,
                                  **options)
      end

      # Get all available strategies
      #
      # @return [Array<Vectory::Conversion::Strategy>] all registered strategies
      def strategies
        @strategies ||= [
          InkscapeStrategy.new,
          GhostscriptStrategy.new,
        ]
      end

      # Get strategies that support a specific conversion
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Array<Vectory::Conversion::Strategy>] matching strategies
      def strategies_for(input_format, output_format)
        strategies.select { |s| s.supports?(input_format, output_format) }
      end

      # Check if a conversion is supported
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Boolean] true if any strategy supports this conversion
      def supports?(input_format, output_format)
        strategies_for(input_format, output_format).any?
      end

      # Get all supported conversions
      #
      # @return [Array<Array<Symbol>>] array of [input, output] format pairs
      def supported_conversions
        @supported_conversions ||= strategies.flat_map(&:supported_conversions).uniq
      end

      # Check if a specific tool is available
      #
      # @param tool [Symbol, String] the tool name (:inkscape, :ghostscript, etc.)
      # @return [Boolean] true if the tool is available
      def tool_available?(tool)
        strategy = strategies.find { |s| s.tool_name == tool.to_s.downcase }
        strategy&.available? || false
      end

      private

      # Find a strategy for the given conversion
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Vectory::Conversion::Strategy, nil] the strategy or nil if not found
      def find_strategy(input_format, output_format)
        strategies.find do |strategy|
          strategy.supports?(input_format, output_format) && strategy.available?
        end
      end
    end
  end
end
