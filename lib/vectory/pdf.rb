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
      svg = InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :svg,
        output_class: Svg,
        plain: true,
      )

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
    end

    def to_ps
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :ps,
        output_class: Ps,
      )
    end

    def to_emf
      InkscapeWrapper.convert(
        content: content,
        input_format: :pdf,
        output_format: :emf,
        output_class: Emf,
      )
    end

    private

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
