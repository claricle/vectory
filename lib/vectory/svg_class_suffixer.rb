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

        rewrite_style(element, suffix)
      end

      def rewrite_style(element, suffix)
        # Parse the complete CSS stream, even when an XML comment or CDATA
        # boundary splits a token. Apply insertions to the original text nodes
        # so comments, processing instructions and CDATA remain in place.
        nodes = element.xpath(".//text()")
        changes = css_insertions(nodes.map(&:text).join, suffix)
        offset = 0
        nodes.each do |node|
          text = node.text
          rewrite_style_node(node, text, offset, changes)
          offset += text.length
        end
      end

      def rewrite_style_node(node, text, offset, changes)
        rewritten = text.dup
        changes.each do |position, insertion|
          next unless position > offset && position <= offset + text.length

          rewritten.insert(position - offset, insertion)
        end
        node.content = rewritten unless rewritten == text
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

      def css_insertions(css, suffix)
        changes = {}
        rules = Crass::Parser.new(css, preserve_comments: true).consume_rules(top_level: true)
        rewrite_rules(rules, suffix, changes)
        # Crass tolerates malformed CSS but stringify may repair or drop it.
        # Token positions use CSS-normalized newlines; map them to the source.
        offsets = source_offsets(css)
        changes.sort.reverse.map { |position, insertion| [offsets[position], insertion] }
      end

      def source_offsets(css)
        offsets = [0]
        css.scan(/\r\n|./m) { |character| offsets << (offsets.last + character.length) }
        offsets
      end

      def rewrite_rules(rules, suffix, changes)
        rules.each do |rule|
          case rule[:node]
          when :qualified_rule
            rewrite_selector(rule[:prelude], suffix, changes)
            rewrite_nested_rules(rule[:block], suffix, changes)
          when :at_rule
            rewrite_at_rule(rule, suffix, changes)
          end
        end
      end

      def rewrite_nested_rules(tokens, suffix, changes)
        # Crass's declaration parser predates CSS nesting and discards nested
        # rules. Walk its component values, keeping declaration values opaque.
        parser = Crass::Parser.new(tokens)
        prelude = []
        while (token = parser.tokens.consume)
          case token[:node]
          when :semicolon
            prelude.clear
          when :at_keyword
            rewrite_nested_at_rule(parser, prelude, suffix, changes)
          when :simple_block
            rewrite_nested_block(token, prelude, suffix, changes)
          else
            prelude << token
          end
        end
      end

      def rewrite_nested_at_rule(parser, prelude, suffix, changes)
        if prelude.all? { |token| trivia?(token) }
          parser.tokens.reconsume
          rewrite_at_rule(parser.consume_at_rule, suffix, changes)
          prelude.clear
        else
          prelude << parser.tokens.current
        end
      end

      def rewrite_nested_block(token, prelude, suffix, changes)
        if token[:start] == "{" && !custom_property?(prelude)
          rewrite_selector(prelude, suffix, changes)
          rewrite_nested_rules(token[:value], suffix, changes)
          prelude.clear
        else
          prelude << token
        end
      end

      def trivia?(token)
        [:comment, :whitespace].include?(token[:node])
      end

      def custom_property?(tokens)
        significant = tokens.reject { |token| trivia?(token) }
        significant.first&.dig(:node) == :ident && significant.first[:value].start_with?("--") &&
          significant[1]&.dig(:node) == :colon
      end

      def rewrite_at_rule(rule, suffix, changes)
        return unless rule[:block] && RULE_AT_RULES.include?(rule[:name].downcase)

        rewrite_scope(rule[:prelude], suffix, changes) if rule[:name].downcase == "scope"
        rewrite_nested_rules(rule[:block], suffix, changes)
      end

      def rewrite_scope(tokens, suffix, changes)
        tokens.each do |token|
          next unless token[:node] == :simple_block && token[:start] == "("

          rewrite_selector(token[:value], suffix, changes)
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

        # A trailing short hex escape can absorb the first suffix character.
        # Add its optional terminator only when concatenation changes meaning.
        combined = Crass::Tokenizer.tokenize(token[:raw] + suffix).first
        insertion = combined[:value] == token[:value] + suffix ? suffix : " #{suffix}"
        changes[token[:pos] + token[:raw].length] = insertion
      end
    end
  end
end
