# frozen_string_literal: true

require_relative "ps2pdf_wrapper"
require_relative "pdf"

module Vectory
  class Ps < Vector
    def self.default_extension
      "ps"
    end

    def self.mimetype
      "application/postscript"
    end

    def self.from_node(node)
      return from_content(node.children.to_xml) unless node.text.strip.empty?

      uri = node["src"]
      return Vectory::Datauri.new(uri).to_vector if %r{^data:}.match?(uri)

      from_path(uri)
    end

    def to_eps
      to_pdf.to_eps
    end

    def to_emf
      to_pdf.to_emf
    end

    def to_svg
      to_pdf.to_svg
    end

    def to_pdf
      pdf_content = Ps2pdfWrapper.convert(content, eps_crop: false)
      pdf = Pdf.new(pdf_content)
      # Pass original BoundingBox dimensions to preserve them in conversions
      bbox = parse_bounding_box
      if bbox
        pdf.original_width = bbox[:urx] - bbox[:llx]
        pdf.original_height = bbox[:ury] - bbox[:lly]
      end
      pdf
    end

    def height
      bbox = parse_bounding_box
      return super unless bbox

      bbox[:ury] - bbox[:lly]
    end

    def width
      bbox = parse_bounding_box
      return super unless bbox

      bbox[:urx] - bbox[:llx]
    end

    private

    def parse_bounding_box
      # Look for %%BoundingBox: llx lly urx ury
      match = content.match(/^%%BoundingBox:\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)/m)
      return nil unless match

      {
        llx: match[1].to_f,
        lly: match[2].to_f,
        urx: match[3].to_f,
        ury: match[4].to_f,
      }
    end
  end
end
