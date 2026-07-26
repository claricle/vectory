# frozen_string_literal: true

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
      InkscapeWrapper.convert(
        content: content,
        input_format: :emf,
        output_format: :eps,
        output_class: Eps,
      )
    end

    def to_ps
      InkscapeWrapper.convert(
        content: content,
        input_format: :emf,
        output_format: :ps,
        output_class: Ps,
      )
    end
  end
end
