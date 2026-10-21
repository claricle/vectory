require "leptris"

module Vectory
  # Disambiguates element ids across multiple SVG documents embedded in one
  # output (the multi-svg collision problem): ids that appear in more than
  # one SVG get a per-document suffix, and every reference form is rewritten
  # to match — `url(#id)` in presentation attributes and `<style>` blocks,
  # `href`/`xlink:href` fragment references, and the `id` attributes
  # themselves.
  #
  # This is the string-in/string-out counterpart of SvgMapping's document
  # processing: content in, processed content out, same as SvgMapping's
  # "serialize on entry" contract. Ported from metanorma-standoc's
  # svg_uniqueids cleanup (lib/metanorma/cleanup/image.rb), where it was
  # written against Nokogiri inline.
  #
  # Usage — N embedded SVGs, disambiguate by position:
  #
  #   dupes = Vectory::SvgIdDisambiguator.duplicate_ids(svgs)
  #   svgs.map.with_index { |svg, i| Vectory::SvgIdDisambiguator.rewrite(svg, "_inject_#{i}", dupes) }
  #
  class SvgIdDisambiguator
    # SVG elements whose ids are referenced through presentation attributes
    # and CSS rather than href: the tag maps to the attributes that carry
    # the reference (nil = the tag name itself is the CSS property).
    IRI_TAG_PROPERTIES_MAP = {
      "clipPath" => ["clip-path"],
      "color-profile" => nil,
      "cursor" => nil,
      "filter" => nil,
      "linearGradient" => ["fill", "stroke"],
      "marker" => ["marker", "marker-end", "marker-mid", "marker-start"],
      "mask" => nil,
      "pattern" => ["fill", "stroke"],
      "radialGradient" => ["fill", "stroke"],
    }.freeze

    URL_RE = %r{url\("?#([a-zA-Z][\w:.-]*)"?\)}.freeze
    private_constant :URL_RE

    class << self
      # Ids that appear more than once across the given SVG contents.
      #
      # @param contents [Array<String>] SVG document contents
      # @return [Hash{String => Integer}] id => occurrence count (count >= 2)
      def duplicate_ids(contents)
        contents.flat_map { |c| id_values(Leptris::XML.parse(c)) }
          .group_by(&:itself).transform_values(&:count)
          .delete_if { |_, v| v < 2 }
      end

      # Suffix every id in +duplicate_ids+ with +suffix+ inside +content+,
      # rewriting all reference forms to match. Ids absent from the map are
      # left untouched.
      #
      # @param content [String] SVG document content
      # @param suffix [String] the full disambiguator, separator included
      #   (e.g. "_inject_0")
      # @param duplicate_ids [Hash{String => Integer}] from .duplicate_ids
      # @return [String] the rewritten content
      def rewrite(content, suffix, duplicate_ids)
        doc = Leptris::XML.parse(content)
        svg_roots(doc).each { |svg| rewrite_svg(svg, suffix, duplicate_ids) }
        doc.to_xml
      end

      private

      def svg_roots(doc)
        root = doc.root
        if root&.name == "svg"
          [root]
        else
          # SVGs embedded in a host document
          doc.xpath("//*[local-name()='svg']")
        end
      end

      def id_values(doc)
        doc.xpath("//*").filter_map { |e| e["id"] if e["id"] }
      end

      def rewrite_svg(svg, suffix, ids)
        id_elems = svg.xpath(".//*[@id]")
        id_elems = [svg] + id_elems if svg["id"]
        iri_properties = iri_properties(id_elems)
        ([svg] + svg.xpath("//*")).each { |e| rewrite_element(e, iri_properties, suffix, ids) }
      end

      def rewrite_element(elem, iri_properties, suffix, ids)
        if elem.name == "style"
          elem.children = rewrite_urls(elem.text, suffix, ids)
          return
        end

        # All attribute reads AND writes go through attribute_nodes: mixing
        # in the elem[...] accessors after an id write desyncs the native
        # attribute layer (leptris-ruby#526).
        nodes = elem.attribute_nodes || []
        return if nodes.empty?

        iri_properties.each do |p|
          node = nodes.find { |a| a.name == p } or next
          node.value = rewrite_urls(node.value, suffix, ids)
        end
        nodes.each do |a|
          case a.name
          when "id"
            a.value = "#{a.value}#{suffix}" if ids[a.value]
          when "xlink:href", "href"
            v = a.value.to_s.strip
            a.value = "#{a.value}#{suffix}" if v.start_with?("#") && ids[v.sub(/\A#/, "")]
          end
        end
      end

      # The CSS-referenceable properties present among the id-bearing
      # elements, plus "style" when any exist (standoc svg_iri_properties).
      def iri_properties(id_elems)
        iri_tag_names = id_elems.filter_map do |e|
          e.name if IRI_TAG_PROPERTIES_MAP.key?(e.name)
        end.uniq
        iri_properties = iri_tag_names.flat_map do |t|
          IRI_TAG_PROPERTIES_MAP[t] || [t]
        end.uniq
        return [] if iri_properties.empty?

        iri_properties + ["style"]
      end

      def rewrite_urls(text, suffix, ids)
        text.gsub(URL_RE) do |x|
          ids[$1] ? "url(##{$1}#{suffix})" : x
        end
      end
    end
  end
end
