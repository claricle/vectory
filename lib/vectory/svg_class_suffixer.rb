# frozen_string_literal: true

require "crass"
require "leptris"

module Vectory
  # Suffixes SVG class lists and CSS class selectors without normalizing CSS.
  class SvgClassSuffixer
    RULE_AT_RULES = %w[media supports layer container document -moz-document
                       scope starting-style keyframes -webkit-keyframes].freeze
    SELECTOR_FUNCTIONS = %w[is where not has nth-child nth-last-child host
                            host-context slotted].freeze
    private_constant :RULE_AT_RULES, :SELECTOR_FUNCTIONS

    class << self
      # @param content [String] SVG XML, or a host document containing SVGs
      # @param suffix [String] a CSS identifier suffix, e.g. "_inject_0"
      # @return [String] rewritten XML
      def rewrite(content, suffix)
        doc = parse(content)
        doc.xpath("//*[local-name()='svg']").each do |svg|
          ([svg] + svg.xpath(".//*")).each do |element|
            rewrite_element(element, suffix)
          end
        end
        doc.to_xml
      end

      private

      def parse(content)
        doc = Leptris::XML.parse(content)
        # Recent Leptris versions recover malformed XML by default.
        raise Leptris::XML::ParseError, doc.errors.first.message if doc.respond_to?(:errors) && !doc.errors.empty?

        doc
      end

      def rewrite_element(element, suffix)
        rewrite_classes(element, suffix)
        return unless element.name == "style"

        css = element.text
        rewritten = rewrite_css(css.dup, suffix)
        element.content = rewritten unless rewritten == css
      end

      def rewrite_classes(element, suffix)
        # Keep attribute reads and writes on Leptris's native attribute layer.
        attribute = element.attribute_nodes.find { |node| node.name == "class" }
        if attribute
          attribute.value = attribute.value.gsub(/[^\t\n\f\r ]+/) do |name|
            name.end_with?(suffix) ? name : "#{name}#{suffix}"
          end
        end
      end

      def rewrite_css(css, suffix)
        changes = {}
        rewrite_rules(Crass.parse(css, preserve_comments: true), suffix, changes)
        # Crass tolerates malformed CSS but stringify may repair or drop it.
        # Token positions use CSS-normalized newlines; map them to the source.
        offsets = source_offsets(css)
        changes.sort.reverse_each do |position, raw|
          css.insert(offsets[position + raw.length - suffix.length], suffix)
        end
        css
      end

      def source_offsets(css)
        offsets = [0]
        css.scan(/\r\n|./m) { |character| offsets << (offsets.last + character.length) }
        offsets
      end

      def rewrite_rules(rules, suffix, changes)
        rules.each do |rule|
          case rule[:node]
          when :style_rule
            rewrite_selector(rule[:selector][:tokens], suffix, changes)
          when :at_rule
            next unless rule[:block] && RULE_AT_RULES.include?(rule[:name].downcase)

            rewrite_rules(Crass::Parser.parse_rules(rule[:block]), suffix, changes)
          end
        end
      end

      def rewrite_selector(tokens, suffix, changes)
        previous = nil
        tokens.each do |token|
          next if token[:node] == :comment

          if class_identifier?(token, previous)
            suffix_identifier(token, suffix, changes)
          elsif selector_function?(token)
            rewrite_selector(token[:value], suffix, changes)
          end
          previous = token
        end
      end

      def class_identifier?(token, previous)
        token[:node] == :ident && previous&.dig(:node) == :delim && previous[:value] == "."
      end

      def selector_function?(token)
        token[:node] == :function && SELECTOR_FUNCTIONS.include?(token[:name].downcase)
      end

      def suffix_identifier(token, suffix, changes)
        return if token[:value].end_with?(suffix)

        token[:value] += suffix
        token[:raw] += suffix
        changes[token[:pos]] = token[:raw]
      end
    end
  end
end
