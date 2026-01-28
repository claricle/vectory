module Vectory
  # Processes SVG mapping in XML documents.
  #
  # SvgMapping integrates SVG content into XML documents by:
  # - Extracting SVG from image tags or inline <svg> elements
  # - Processing <target> elements to build link mappings
  # - Applying ID suffixes for uniqueness in multi-document scenarios
  # - Simplifying svgmap structure for final output
  #
  # == ID Suffixing
  #
  # SVG IDs are suffixed in two stages to ensure uniqueness:
  #
  # 1. **ID suffix** (optional) - Derived from document/container identity
  #    Provides cross-document uniqueness (e.g., <id>_ISO_17301-1_2016)
  #
  # 2. **Index suffix** - Derived from svgmap position in document
  #    Provides multi-svgmap uniqueness (e.g., <id>_000000000)
  #
  # Final ID: <original_id><id_suffix><index_suffix>
  # Example: fig1_ISO_17301-1_2016_000000000
  #
  # @example Basic usage
  #   xml_string = Vectory::SvgMapping.from_path("doc.xml").to_xml
  #
  # @example With ID suffix for multi-document uniqueness
  #   mapping = Vectory::SvgMapping.new(doc, "", id_suffix: "_ISO_17301-1_2016")
  #   xml_string = mapping.call.to_xml
  #
  class SvgMapping
    # Namespace helper for XML namespace handling
    class Namespace
      # @param xmldoc [Nokogiri::XML::Document] The XML document
      def initialize(xmldoc)
        @namespace = xmldoc.root.namespace
      end

      # Converts XPath to use document namespace
      # @param path [String] The XPath expression
      # @return [String] XPath with namespace prefixes
      def ns(path)
        return path if @namespace.nil?

        path.gsub(%r{/([a-zA-z])}, "/xmlns:\\1")
          .gsub(%r{::([a-zA-z])}, "::xmlns:\\1")
          .gsub(%r{\[([a-zA-z][a-z0-9A-Z@/]* ?=)}, "[xmlns:\\1")
          .gsub(%r{\[([a-zA-z][a-z0-9A-Z@/]*\])}, "[xmlns:\\1")
      end
    end

    SVG_NS = "http://www.w3.org/2000/svg".freeze
    private_constant :SVG_NS

    # XPath to remove processing instructions during SVG processing
    PROCESSING_XPATH =
      "processing-instruction()|.//processing-instruction()".freeze
    private_constant :PROCESSING_XPATH

    # Creates an SvgMapping from an XML file path.
    #
    # @param path [String] Path to the XML file
    # @return [Vectory::SvgMapping] New mapping instance
    def self.from_path(path)
      new(Nokogiri::XML(File.read(path)))
    end

    # Creates an SvgMapping from an XML string.
    #
    # @param xml [String] XML content
    # @return [Vectory::SvgMapping] New mapping instance
    def self.from_xml(xml)
      new(Nokogiri::XML(xml))
    end

    # Initializes a new SvgMapping processor.
    #
    # @param doc [Nokogiri::XML::Document] The document containing svgmap elements
    # @param local_directory [String] Directory for resolving relative file paths
    # @param id_suffix [String, nil] Optional suffix derived from document/container
    #   identity. Applied before the index suffix to provide cross-document
    #   uniqueness. Example: "_ISO_17301-1_2016"
    def initialize(doc, local_directory = "", id_suffix: nil)
      @doc = doc
      @local_directory = local_directory
      @id_suffix = id_suffix
    end

    # Processes all svgmap elements in the document.
    #
    # @return [Nokogiri::XML::Document] The processed document
    def call
      @namespace = Namespace.new(@doc)

      @doc.xpath(@namespace.ns("//svgmap")).each_with_index do |svgmap, index|
        process_svgmap(svgmap, index)
      end

      @doc
    end

    # Processes and returns XML string.
    #
    # @return [String] XML representation of processed document
    def to_xml
      call.to_xml
    end

    private

    # Processes a single svgmap element.
    #
    # @param svgmap [Nokogiri::XML::Element] The svgmap element to process
    # @param index [Integer] Position of this svgmap in the document (0-indexed)
    def process_svgmap(svgmap, index)
      image = extract_image_tag(svgmap)
      return unless image

      content = generate_content(image, svgmap, index)
      return unless content

      image.replace(content)

      simplify_svgmap(svgmap)
    end

    # Extracts the image element (SVG inline or image with src) from svgmap.
    #
    # @param svgmap [Nokogiri::XML::Element] The svgmap element
    # @return [Nokogiri::XML::Element, nil] The image/svg element or nil
    def extract_image_tag(svgmap)
      image = svgmap.at(@namespace.ns(".//image"))
      return image if image && image["src"] && !image["src"].empty?

      svgmap.at(".//m:svg", "m" => SVG_NS)
    end

    # Generates processed SVG content for an svgmap.
    #
    # @param image [Nokogiri::XML::Element] The image/svg element
    # @param svgmap [Nokogiri::XML::Element] The parent svgmap element
    # @param index [Integer] Index suffix to apply
    # @return [String, nil] Processed SVG content or nil if invalid
    def generate_content(image, svgmap, index)
      document = build_svg_document(image)
      return unless document

      links_map = from_targets_to_links_map(svgmap)
      # Pass nil for id_suffix here; it's already handled in SvgDocument.namespace
      document.namespace(index, links_map, PROCESSING_XPATH, id_suffix: @id_suffix)

      document.content
    end

    # Builds an SvgDocument from an image element.
    #
    # @param image [Nokogiri::XML::Element] The image element
    # @return [Vectory::SvgDocument, nil] The SVG document or nil if invalid
    def build_svg_document(image)
      vector = build_vector(image)
      return unless vector

      SvgDocument.new(vector.content)
    end

    # Builds a Vector from an image element.
    #
    # @param image [Nokogiri::XML::Element] The image element
    # @return [Vectory::Vector, nil] The vector or nil if invalid
    def build_vector(image)
      return Vectory::Svg.from_content(image.to_xml) if image.name == "svg"

      return unless image.name == "image"

      src = image["src"]
      return Vectory::Datauri.new(src).to_vector if /^data:/.match?(src)

      path = @local_directory.empty? ? src : File.join(@local_directory, src)
      return unless File.exist?(path)

      Vectory::Svg.from_path(path)
    end

    # Builds a mapping from target hrefs to their link targets.
    #
    # @param svgmap [Nokogiri::XML::Element] The svgmap element
    # @return [Hash{String => String}] Mapping of href to target
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

    # Extracts link target from a target element.
    #
    # @param target_tag [Nokogiri::XML::Element] The target element
    # @return [String, nil] The link target or nil
    def link_target(target_tag)
      xref = target_tag.at(@namespace.ns("./xref"))
      return "##{xref['target']}" if xref

      link = target_tag.at(@namespace.ns("./link"))
      return unless link

      link["target"]
    end

    # Simplifies svgmap after processing.
    # Removes svgmap wrapper if no more target elements with eref.
    #
    # @param svgmap [Nokogiri::XML::Element] The svgmap element
    def simplify_svgmap(svgmap)
      return if svgmap.at(@namespace.ns("./target/eref"))

      svgmap.replace(svgmap.at(@namespace.ns("./figure")))
    end
  end
end
