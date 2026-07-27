# frozen_string_literal: true

require "nokogiri"
require "postsvg"

module Vectory
  class Svg < Vector
    SVG_NS = "http://www.w3.org/2000/svg"

    def self.default_extension
      "svg"
    end

    def self.mimetype
      "image/svg+xml"
    end

    def self.from_node(node)
      if node.elements&.first&.name == "svg"
        return from_content(node.children.to_xml)
      end

      uri = node["src"]
      return Vectory::Datauri.new(uri).to_vector if %r{^data:}.match?(uri)

      from_path(uri)
    end

    def initialize(content = nil, initial_path = nil)
      super

      self.content = content
    end

    def to_emf
      Emf.from_content(Emfsvg.from_svg(content))
    end

    def to_eps
      Eps.from_content(Postsvg.to_eps(content))
    end

    def to_ps
      Ps.from_content(Postsvg.to_ps(content))
    end

    def height
      dim_from_attr("height") || dim_from_viewbox(3) || super
    end

    def width
      dim_from_attr("width") || dim_from_viewbox(2) || super
    end

    private

    def svg_root
      doc = Nokogiri::XML(content)
      doc.at_xpath("//svg:svg", "svg" => SVG_NS) || doc.at_xpath("//svg")
    end

    def dim_from_attr(name)
      value = svg_root&.[](name)
      value&.to_f&.round
    end

    def dim_from_viewbox(index)
      vb = svg_root&.[]("viewBox")
      return nil unless vb

      parts = vb.split
      return nil unless parts.length == 4

      parts[index].to_f.round
    end

    def content=(content)
      # non-root node inserts the xml tag which breaks markup when placed in
      # another xml document
      document = Nokogiri::XML(content).root
      unless document
        raise ParsingError, "Could not parse '#{content&.slice(0, 30)}'"
      end

      @content = document.to_xml
    end
  end
end
