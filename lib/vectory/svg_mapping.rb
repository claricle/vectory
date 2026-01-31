module Vectory
  class SvgMapping
    class Namespace
      def initialize(xmldoc)
        @namespace = xmldoc.root.namespace
      end

      def ns(path)
        return path if @namespace.nil?

        path.gsub(%r{/([a-zA-z])}, "/xmlns:\\1")
          .gsub(%r{::([a-zA-z])}, "::xmlns:\\1")
          .gsub(%r{\[([a-zA-z][a-z0-9A-Z@/]* ?=)}, "[xmlns:\\1")
          .gsub(%r{\[([a-zA-z][a-z0-9A-Z@/]*\])}, "[xmlns:\\1")
      end
    end

    SVG_NS = "http://www.w3.org/2000/svg".freeze
    PROCESSING_XPATH =
      "processing-instruction()|.//processing-instruction()".freeze

    def self.from_path(path)
      new(Nokogiri::XML(File.read(path)))
    end

    def self.from_xml(xml)
      new(Nokogiri::XML(xml))
    end

    def initialize(doc, local_directory = "")
      @doc = doc
      @local_directory = local_directory
    end

    def call
      @namespace = Namespace.new(@doc)

      @doc.xpath(@namespace.ns("//svgmap")).each_with_index do |svgmap, index|
        process_svgmap(svgmap, index)
      end

      @doc
    end

    def to_xml
      call.to_xml
    end

    private

    def process_svgmap(svgmap, suffix)
      image = extract_image_tag(svgmap)
      return unless image

      content = generate_content(image, svgmap, suffix)
      return unless content

      image.replace(content)

      simplify_svgmap(svgmap)
    end

    def extract_image_tag(svgmap)
      image = svgmap.at(@namespace.ns(".//image"))
      return image if image && image["src"] && !image["src"].empty?

      svgmap.at(".//m:svg", "m" => SVG_NS)
    end

    def generate_content(image, svgmap, suffix)
      document = build_svg_document(image)
      return unless document

      links_map = from_targets_to_links_map(svgmap)
      document.namespace(suffix, links_map, PROCESSING_XPATH)

      document.content
    end

    def build_svg_document(image)
      vector = build_vector(image)
      return unless vector

      SvgDocument.new(vector.content)
    end

    def build_vector(image)
      return Vectory::Svg.from_content(image.to_xml) if image.name == "svg"

      return unless image.name == "image"

      src = image["src"]
      return Vectory::Datauri.new(src).to_vector if /^data:/.match?(src)

      path = @local_directory.empty? ? src : File.join(@local_directory, src)
      return unless File.exist?(path)

      Vectory::Svg.from_path(path)
    end

    def from_targets_to_links_map(svgmap)
      targets = svgmap.xpath(@namespace.ns("./target"))
      targets.each_with_object({}) do |target_tag, m|
        target = link_target(target_tag)
        next unless target

        href = File.expand_path(target_tag["href"])
        m[href] = target

        target_tag.remove
      end
    end

    def link_target(target_tag)
      xref = target_tag.at(@namespace.ns("./xref"))
      return "##{xref['target']}" if xref

      link = target_tag.at(@namespace.ns("./link"))
      return unless link

      link["target"]
    end

    def simplify_svgmap(svgmap)
      return if svgmap.at(@namespace.ns("./target/eref"))

      svgmap.replace(svgmap.at(@namespace.ns("./figure")))
    end
  end
end
