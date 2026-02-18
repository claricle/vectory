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
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :eps,
        output_class: Eps,
      )
    rescue Vectory::ConversionError => e
      if e.message.include?("Output file not found")
        # Fallback: PDF → EPS (Ghostscript) → EPS (Inkscape)
        warn "[VECTORY DEBUG] PDF direct import failed, trying PDF → EPS → EPS fallback" if ENV["VECTORY_DEBUG"]
        intermediate_eps = GhostscriptWrapper.pdf_to_eps(content)
        return InkscapeWrapper.convert(
          content: intermediate_eps,
          input_format: :eps,
          output_format: :eps,
          output_class: Eps,
        )
      end

      raise
    end

    def to_ps
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :ps,
        output_class: Ps,
      )
    rescue Vectory::ConversionError => e
      if e.message.include?("Output file not found")
        # Fallback: PDF → EPS (Ghostscript) → PS (Inkscape)
        warn "[VECTORY DEBUG] PDF direct import failed, trying PDF → EPS → PS fallback" if ENV["VECTORY_DEBUG"]
        intermediate_eps = GhostscriptWrapper.pdf_to_eps(content)
        return InkscapeWrapper.convert(
          content: intermediate_eps,
          input_format: :eps,
          output_format: :ps,
          output_class: Ps,
        )
      end

      raise
    end

    def to_emf
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :emf,
        output_class: Emf,
      )
    rescue Vectory::ConversionError => e
      if e.message.include?("Output file not found")
        # Fallback: PDF → EPS (Ghostscript) → EMF (Inkscape)
        warn "[VECTORY DEBUG] PDF direct import failed, trying PDF → EPS → EMF fallback" if ENV["VECTORY_DEBUG"]
        intermediate_eps = GhostscriptWrapper.pdf_to_eps(content)
        return InkscapeWrapper.convert(
          content: intermediate_eps,
          input_format: :eps,
          output_format: :emf,
          output_class: Emf,
        )
      end

      raise
    end

    private

    # Convert PDF to SVG, trying Inkscape first with Ghostscript fallback
    #
    # Inkscape 1.4.x on Windows and macOS has a PDF import bug where it
    # returns exit code 0 but doesn't create the output file.
    #
    # Fallback: Use Ghostscript to convert PDF → EPS, then Inkscape for EPS → SVG.
    # This two-step process works because Inkscape's EPS import uses Ghostscript
    # internally, and the combination is more reliable than direct PDF import.
    #
    # @return [Vectory::Svg] the converted SVG
    # @raise [Vectory::ConversionError] if both methods fail
    def convert_to_svg
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :svg,
        output_class: Svg,
        plain: true,
      )
    rescue Vectory::ConversionError => e
      # Check if this is the "Output file not found" error (Inkscape PDF import bug)
      if e.message.include?("Output file not found")
        # Fall back to PDF → EPS (Ghostscript) → SVG (Inkscape)
        warn "[VECTORY DEBUG] PDF direct import failed, trying PDF → EPS → SVG fallback" if ENV["VECTORY_DEBUG"]
        eps_content = GhostscriptWrapper.pdf_to_eps(content)
        warn "[VECTORY DEBUG] PDF → EPS conversion succeeded, now trying EPS → SVG" if ENV["VECTORY_DEBUG"]
        return InkscapeWrapper.convert(
          content: eps_content,
          input_format: :eps,
          output_format: :svg,
          output_class: Svg,
          plain: true,
        )
      end

      raise
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
