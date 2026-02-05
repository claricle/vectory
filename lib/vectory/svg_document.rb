require "nokogiri"

module Vectory
  # Represents and processes an SVG document within XML.
  #
  # SvgDocument handles SVG ID suffixing for uniqueness in multi-document
  # and multi-svgmap scenarios. It applies two types of suffixes:
  #
  # 1. **ID suffix** - Derived from document/container identity
  #    Provides cross-document uniqueness
  #
  # 2. **Index suffix** - Derived from svgmap position
  #    Provides multi-svgmap uniqueness within a document
  #
  # Final ID format: <original_id><id_suffix><index_suffix>
  # Example: fig1 + _ISO_17301-1_2016 + _000000000 = fig1_ISO_17301-1_2016_000000000
  #
  class SvgDocument
    SVG_NS = "http://www.w3.org/2000/svg".freeze

    class << self
      # Updates ID references in CSS style statements within a document.
      #
      # @param document [Nokogiri::XML::Document] The SVG document
      # @param ids [Array<String>] IDs to update
      # @param suffix [Integer, String] The suffix to apply (integers are zero-padded)
      # @return [void]
      def update_ids_css(document, ids, suffix)
        suffix = suffix.is_a?(Integer) ? sprintf("%09d", suffix) : suffix
        document.xpath(".//m:style", "m" => SVG_NS).each do |s|
          c = s.children.to_xml
          s.children = update_ids_css_string(c, ids, suffix)
        end
        document.xpath(".//*[@style]").each do |s|
          s["style"] = update_ids_css_string(s["style"], ids, suffix)
        end
      end

      # Updates ID references in a CSS style string.
      #
      # @param style [String] The CSS style content
      # @param ids [Array<String>] IDs to update
      # @param suffix [Integer, String] The suffix to apply (integers are zero-padded)
      # @return [String] Updated style content
      def update_ids_css_string(style, ids, suffix)
        ids.each do |i|
          style = style.gsub(%r[##{i}\b],
                             sprintf("#%<id>s_%<suffix>s", id: i,
                                                           suffix: suffix))
            .gsub(%r(\[id\s*=\s*['"]?#{i}['"]?\]),
                  sprintf("[id='%<id>s_%<suffix>s']", id: i,
                                                      suffix: suffix))
        end
        style
      end

      # Updates ID attributes in SVG elements.
      #
      # @param document [Nokogiri::XML::Document] The SVG document
      # @param ids [Array<String>] IDs to update
      # @param suffix [Integer, String] The suffix to apply (integers are zero-padded)
      # @return [void]
      def update_ids_attrs(document, ids, suffix)
        suffix = suffix.is_a?(Integer) ? sprintf("%09d", suffix) : suffix
        document.xpath(". | .//*[@*]").each do |a|
          a.attribute_nodes.each do |x|
            val = x.value.sub(/^#/, "")
            ids.include?(val) and x.value += "_#{suffix}"
            x.value = x.value.sub(%r{url\(#([^()]+)\)}, "url(#\\1_#{suffix})")
          end
        end
      end
    end

    # Creates a new SvgDocument from SVG content.
    #
    # @param content [String] SVG XML content
    def initialize(content)
      @document = Nokogiri::XML(content)
    end

    # Returns the processed SVG content as XML string.
    #
    # @return [String] SVG XML content
    def content
      @document.root.to_xml
    end

    # Applies namespace transformations to the SVG document.
    #
    # This method performs the following operations in order:
    # 1. Remap internal links to their targets
    # 2. Apply ID suffix (if provided) for cross-document uniqueness
    # 3. Apply index suffix for multi-svgmap uniqueness
    # 4. Remove processing instructions
    #
    # @param index_suffix [Integer] The position-based suffix (0, 1, 2...)
    #   Converted to 9-digit zero-padded string (e.g., "_000000000")
    # @param links [Hash{String => String}] Mapping of link hrefs to their targets
    # @param xpath_to_remove [String] XPath for elements to remove (processing instructions)
    # @param id_suffix [String, nil] Optional suffix derived from document/container
    #   identity. Applied BEFORE index_suffix for two-stage disambiguation.
    #   Example: "_ISO_17301-1_2016"
    #
    # @return [self] The processed document
    def namespace(index_suffix, links, xpath_to_remove, id_suffix: nil)
      remap_links(links)

      # Apply ID suffix first (cross-document uniqueness)
      # Then apply index suffix (multi-svgmap uniqueness)
      # Final format: <original_id><id_suffix><index_suffix>
      if id_suffix
        apply_id_suffix(id_suffix)
      end

      apply_index_suffix(index_suffix)
      remove_xpath(xpath_to_remove)

      self
    end

    # Remaps internal SVG links to their target definitions.
    #
    # @param map [Hash{String => String}] Mapping of expanded hrefs to targets
    # @return [self] The document
    def remap_links(map)
      @document.xpath(".//m:a", "m" => SVG_NS).each do |a|
        href_attrs = ["xlink:href", "href"]
        href_attrs.each do |p|
          a[p] and x = map[File.expand_path(a[p])] and a[p] = x
        end
      end

      self
    end

    # Applies an ID suffix to all SVG IDs for cross-document uniqueness.
    #
    # This is called BEFORE index-based suffixing.
    #
    # @param id_suffix [String] The suffix to apply (e.g., "_ISO_17301-1_2016")
    # @return [self] The document
    def apply_id_suffix(id_suffix)
      ids = collect_ids
      return if ids.empty?

      self.class.update_ids_attrs(@document.root, ids, id_suffix)
      self.class.update_ids_css(@document.root, ids, id_suffix)

      self
    end

    # Applies an index-based suffix to all SVG IDs.
    #
    # The index is converted to a 9-digit zero-padded string.
    # This is called AFTER ID-based suffixing.
    #
    # @param index [Integer] The index suffix (converted to "_000000000" format)
    # @return [self] The document
    def apply_index_suffix(index)
      ids = collect_ids
      return if ids.empty?

      self.class.update_ids_attrs(@document.root, ids, index)
      self.class.update_ids_css(@document.root, ids, index)

      self
    end

    # Applies an index-based suffix to all SVG IDs.
    #
    # @deprecated Use {#apply_index_suffix} instead for clarity.
    # Maintained for backward compatibility.
    #
    # @param suffix [Integer, String] The suffix to apply (integers are zero-padded)
    # @return [self] The document
    def suffix_ids(suffix)
      apply_index_suffix(suffix)
    end

    # Removes elements matching the given XPath.
    #
    # @param xpath [String] XPath expression for elements to remove
    # @return [self] The document
    def remove_xpath(xpath)
      @document.xpath(xpath).remove

      self
    end

    private

    # Collects all id attribute values from the SVG document.
    #
    # @return [Array<String>] All ID values
    def collect_ids
      @document.xpath("./@id | .//@id").map(&:value)
    end
  end
end
