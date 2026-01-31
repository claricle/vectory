# frozen_string_literal: true

module Vectory
  module Conversion
    # Inkscape-based conversion strategy
    #
    # Handles conversions using Inkscape, including:
    # - SVG ↔ EPS
    # - SVG ↔ PS
    # - SVG ↔ EMF
    # - SVG ↔ PDF
    # - EPS/PS → PDF
    #
    # @see https://inkscape.org/
    class InkscapeStrategy < Strategy
      # Inkscape supports bidirectional conversion with SVG as source/target
      SUPPORTED_CONVERSIONS = [
        %i[svg eps],
        %i[svg ps],
        %i[svg emf],
        %i[svg pdf],
        %i[eps svg],
        %i[eps pdf],
        %i[ps svg],
        %i[ps pdf],
        %i[pdf svg],
        %i[emf svg],
      ].freeze

      # Convert content using Inkscape
      #
      # @param content [String] the input content to convert
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @param options [Hash] additional options
      # @option options [Boolean] :plain export plain SVG (for SVG output)
      # @option options [Class] :output_class the class to instantiate with result
      # @return [Vectory::Vector] the converted vector object
      # @raise [Vectory::InkscapeNotFoundError] if Inkscape is not available
      def convert(content, input_format:, output_format:, **options)
        output_class = options.fetch(:output_class) do
          format_class(output_format)
        end

        InkscapeWrapper.convert(
          content: content,
          input_format: input_format,
          output_format: output_format,
          output_class: output_class,
          plain: options[:plain] || false,
        )
      end

      # Check if this conversion is supported
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Boolean] true if Inkscape supports this conversion
      def supports?(input_format, output_format)
        SUPPORTED_CONVERSIONS.include?([input_format, output_format])
      end

      # Get supported conversions
      #
      # @return [Array<Array<Symbol>>] array of [input, output] format pairs
      def supported_conversions
        SUPPORTED_CONVERSIONS
      end

      # Check if Inkscape is available
      #
      # @return [Boolean] true if Inkscape can be found in PATH
      def available?
        InkscapeWrapper.instance.send(:inkscape_path)
        true
      rescue Vectory::InkscapeNotFoundError
        false
      end

      # Get the tool name
      #
      # @return [String] "inkscape"
      def tool_name
        "inkscape"
      end

      # Query the width of content
      #
      # @param content [String] the vector content
      # @param format [Symbol] the format of the content
      # @return [Integer] the width in pixels
      def width(content, format)
        InkscapeWrapper.instance.width(content, format)
      end

      # Query the height of content
      #
      # @param content [String] the vector content
      # @param format [Symbol] the format of the content
      # @return [Integer] the height in pixels
      def height(content, format)
        InkscapeWrapper.instance.height(content, format)
      end

      private

      # Get the Vectory class for a format
      def format_class(format)
        case format
        when :svg then Vectory::Svg
        when :eps then Vectory::Eps
        when :ps then Vectory::Ps
        when :emf then Vectory::Emf
        when :pdf then Vectory::Pdf
        else
          raise ArgumentError, "Unsupported format: #{format}"
        end
      end
    end
  end
end
