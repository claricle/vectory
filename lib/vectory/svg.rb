# frozen_string_literal: true

require "nokogiri"

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
      InkscapeConverter.convert(
        content: content,
        input_format: :svg,
        output_format: :emf,
        output_class: Emf,
      )
    end

    def to_eps
      InkscapeConverter.convert(
        content: content,
        input_format: :svg,
        output_format: :eps,
        output_class: Eps,
      )
    end

    def to_ps
      InkscapeConverter.convert(
        content: content,
        input_format: :svg,
        output_format: :ps,
        output_class: Ps,
      )
    end

    def height
      # Try to read height from SVG attributes first
      doc = Nokogiri::XML(content)
      svg_element = doc.at_xpath("//svg:svg",
                                 "svg" => SVG_NS) || doc.at_xpath("//svg")

      if svg_element && svg_element["height"]
        svg_element["height"].to_f.round
      else
        # Fall back to Inkscape query if no height attribute
        super
      end
    end

    def width
      # Try to read width from SVG attributes first
      doc = Nokogiri::XML(content)
      svg_element = doc.at_xpath("//svg:svg",
                                 "svg" => SVG_NS) || doc.at_xpath("//svg")

      if svg_element && svg_element["width"]
        svg_element["width"].to_f.round
      else
        # Fall back to Inkscape query if no width attribute
        super
      end
    end

    private

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
