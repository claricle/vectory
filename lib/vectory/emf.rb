# frozen_string_literal: true

require "emf2svg"

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
      Dir.mktmpdir do |dir|
        input_path = File.join(dir, "image.emf")
        File.binwrite(input_path, content)

        svg_content = Emf2svg.from_file(input_path)
        Svg.from_content(svg_content)
      end
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
