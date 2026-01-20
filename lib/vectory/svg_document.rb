require "nokogiri"

module Vectory
  class SvgDocument
    SVG_NS = "http://www.w3.org/2000/svg".freeze

    class << self
      # Update instances of id in style statements in a nokogiri document
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

      # Update instances of id in style statements in the string style
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

    def initialize(content)
      @document = Nokogiri::XML(content)
    end

    def content
      @document.root.to_xml
    end

    def namespace(suffix, links, xpath_to_remove)
      remap_links(links)
      suffix_ids(suffix)
      remove_xpath(xpath_to_remove)
    end

    def remap_links(map)
      @document.xpath(".//m:a", "m" => SVG_NS).each do |a|
        href_attrs = ["xlink:href", "href"]
        href_attrs.each do |p|
          a[p] and x = map[File.expand_path(a[p])] and a[p] = x
        end
      end

      self
    end

    def suffix_ids(suffix)
      ids = collect_ids
      return if ids.empty?

      self.class.update_ids_attrs(@document.root, ids, suffix)
      self.class.update_ids_css(@document.root, ids, suffix)

      self
    end

    def remove_xpath(xpath)
      @document.xpath(xpath).remove

      self
    end

    private

    def collect_ids
      @document.xpath("./@id | .//@id").map(&:value)
    end
  end
end
