# frozen_string_literal: true

module Vectory
  class Pdf < Vector
    attr_accessor :original_height, :original_width

    def self.default_extension
      "pdf"
    end

    def self.mimetype
      "application/pdf"
    end

    def to_svg
      svg = convert_to_svg

      # If we have original dimensions from EPS/PS, adjust the SVG
      if original_height && original_width
        adjusted_content = adjust_svg_dimensions(svg.content, original_width,
                                                 original_height)
        svg = Svg.new(adjusted_content, svg.initial_path)
      end

      svg
    end

    def to_eps
      with_inkscape_pdf_fallback(:eps, Eps)
    end

    def to_ps
      with_inkscape_pdf_fallback(:ps, Ps)
    end

    def to_emf
      with_inkscape_pdf_fallback(:emf, Emf)
    end

    private

    # Execute a conversion with fallback for Inkscape PDF import issues
    #
    # Inkscape 1.4.x on Windows and macOS has a PDF import bug where it
    # may fail to create output files. This method catches any conversion
    # error and retries via the PDF → EPS → target format path.
    #
    # @param output_format [Symbol] the target format (:svg, :eps, :ps, :emf)
    # @param output_class [Class] the output class (Svg, Eps, Ps, Emf)
    # @param plain [Boolean] whether to use plain SVG format (only for SVG)
    # @return [Vector] the converted output
    # @raise [Vectory::ConversionError] if both methods fail
    def with_inkscape_pdf_fallback(output_format, output_class, plain: false)
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: output_format,
        output_class: output_class,
        plain: plain,
      )
    rescue Vectory::ConversionError => e
      log_conversion_failure(e, output_format)

      # Try fallback: PDF → EPS (Ghostscript) → target format (Inkscape)
      begin
        warn "[VECTORY] Attempting fallback: PDF → EPS → #{output_format.upcase}" if fallback_logging_enabled?
        eps_content = GhostscriptWrapper.pdf_to_eps(content)
        warn "[VECTORY] PDF → EPS succeeded, now trying EPS → #{output_format.upcase}" if fallback_logging_enabled?
        InkscapeWrapper.convert(
          content: eps_content,
          input_format: :eps,
          output_format: output_format,
          output_class: output_class,
          plain: plain,
        )
      rescue StandardError => fallback_error
        # Wrap non-Vectory errors in ConversionError for consistent error handling
        error_to_raise = if fallback_error.is_a?(Vectory::Error)
                           fallback_error
                         else
                           ConversionError.new(
                             "PDF fallback conversion failed: #{fallback_error.message}",
                           )
                         end
        warn "[VECTORY] Fallback also failed: #{fallback_error.message[0..100]}" if fallback_logging_enabled?
        raise error_to_raise
      end
    end

    # Convert PDF to SVG using fallback mechanism
    def convert_to_svg
      with_inkscape_pdf_fallback(:svg, Svg, plain: true)
    end

    def log_conversion_failure(error, output_format)
      return unless fallback_logging_enabled?

      warn "[VECTORY] PDF → #{output_format.upcase} direct conversion failed:"
      warn "[VECTORY]   Error: #{error.message[0..200]}"
    end

    def fallback_logging_enabled?
      ENV["VECTORY_DEBUG"] || ENV["CI"]
    end

    def adjust_svg_dimensions(svg_content, width, height)
      # Replace width and height attributes in SVG root element
      svg_content.gsub(/(<svg[^>]*\s)width="[^"]*"/, "\\1width=\"#{width}\"")
        .gsub(/(<svg[^>]*\s)height="[^"]*"/, "\\1height=\"#{height}\"")
        .gsub(/(<svg[^>]*\s)viewBox="[^"]*"/) do |match|
          # Adjust viewBox to match new dimensions
          "#{match.split('viewBox')[0]}viewBox=\"0 0 #{width} #{height}\""
      end
    end
  end
end
