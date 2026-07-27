# frozen_string_literal: true

require "postsvg"

module Vectory
  class Eps < Vector
    def self.default_extension
      "eps"
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

    def to_ps
      to_svg.to_ps
    end

    def to_svg
      Svg.from_content(Postsvg.convert(content))
    end

    def to_emf
      to_svg.to_emf
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

    def imgfile_suffix(uri, suffix)
      "#{File.join(File.dirname(uri), File.basename(uri, '.*'))}.#{suffix}"
    end
  end
end
