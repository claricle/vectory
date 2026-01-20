# frozen_string_literal: true

module Vectory
  module Conversion
    # Base class for conversion strategies
    #
    # Conversion strategies encapsulate the logic for converting between
    # different vector formats using external tools (Inkscape, Ghostscript, etc.)
    #
    # @abstract Subclasses must implement the {#convert} method
    class Strategy
      # Convert content from one format to another
      #
      # @param content [String] the input content to convert
      # @param input_format [Symbol] the input format (e.g., :svg, :eps, :ps)
      # @param output_format [Symbol] the desired output format (e.g., :svg, :pdf, :eps)
      # @param options [Hash] additional options for the conversion
      # @return [String] the converted content
      # @raise [Vectory::ConversionError] if conversion fails
      # @abstract
      def convert(content, input_format:, output_format:, **options)
        raise NotImplementedError,
              "#{self.class} must implement #convert method"
      end

      # Check if this strategy supports the given conversion
      #
      # @param input_format [Symbol] the input format
      # @param output_format [Symbol] the output format
      # @return [Boolean] true if this strategy supports the conversion
      def supports?(input_format, output_format)
        supported_conversions.include?([input_format, output_format])
      end

      # Get the list of conversions this strategy supports
      #
      # @return [Array<Array<Symbol>>] array of [input, output] format pairs
      def supported_conversions
        []
      end

      # Check if the required external tool is available
      #
      # @return [Boolean] true if the tool is available
      def available?
        raise NotImplementedError,
              "#{self.class} must implement #available? method"
      end

      # Get the name of the external tool used by this strategy
      #
      # @return [String] the tool name
      def tool_name
        self.class.name.split('::').last.gsub(/Strategy$/, '').downcase
      end
    end
  end
end
