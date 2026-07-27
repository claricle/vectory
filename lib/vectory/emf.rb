# frozen_string_literal: true

require "emf"
require "emfsvg"

module Vectory
  class Emf < Vector
    def self.default_extension
      "emf"
    end

    def self.all_mimetypes
      [mimetype] + alternative_mimetypes
    end

    def self.mimetype
      "image/emf"
    end

    def self.alternative_mimetypes
      ["application/x-msmetafile"]
    end

    def self.from_node(node)
      uri = node["src"]
      return Vectory::Datauri.new(uri).to_vector if %r{^data:}.match?(uri)

      from_path(uri)
    end

    def to_svg
      Svg.from_content(Emfsvg.from_bytes(content))
    end

    def to_eps
      to_svg.to_eps
    end

    def to_ps
      to_svg.to_ps
    end

    def height
      bounds = parse_header_bounds
      return super unless bounds

      bounds.bottom - bounds.top
    end

    def width
      bounds = parse_header_bounds
      return super unless bounds

      bounds.right - bounds.left
    end

    private

    def parse_header_bounds
      ::Emf.parse(content).header.bounds
    rescue StandardError
      nil
    end
  end
end
