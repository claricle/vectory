# frozen_string_literal: true

require "leptris"

module Vectory
  # Only the supported whole-document and selected-SVG profiles are executable.
  class SvgTransformPipeline
    PROFILES = {
      graft: %i[graft_into],
      svg_cleanup: %i[disambiguate suffix_classes bind_namespaces],
      graft_then_svg_cleanup: %i[graft_into disambiguate suffix_classes bind_namespaces],
    }.transform_values(&:freeze).freeze

    attr_reader :steps

    def initialize(steps)
      unless steps.all?(SvgTransform)
        raise ArgumentError, "invalid step sequence: expected SVG transforms"
      end

      @steps = steps.dup.freeze
      validate_sequence
      freeze
    end

    def >>(other)
      additions = other.is_a?(SvgTransformPipeline) ? other.steps : [other]
      self.class.new(steps + additions)
    end

    def profile
      PROFILES.key(steps.map(&:name))
    end

    def call(content)
      raise ArgumentError, "incomplete profile: #{steps.map(&:name).join(' -> ')}" unless profile

      validate_scope(content)
      output = steps.reduce(content) { |xml, step| step.apply(xml) }
      profile == :graft_then_svg_cleanup ? finalize_graft(output) : output
    end

    private

    def finalize_graft(output)
      # Cleanup serializers may reorder the envelope. Keep the cleaned root,
      # including its new attributes and bindings, inside the captured envelope.
      host_content = steps.first.options.fetch(:host_content)
      host = Leptris::XML.parse(host_content, strict: true)
      prefix = host_content.byteslice(0, host.root.byte_offset)
      tail = host_content[/(?:[ \t\r\n]|<!--(?:(?!-->).)*-->|<\?(?:(?!\?>).)*\?>)*\z/m]
      "#{prefix}#{final_graft_root(output).to_xml}#{tail}"
    end

    def final_graft_root(output)
      root = Leptris::XML.parse(output, strict: true).root
      # Bindings sealed onto grafted children may now be supplied by the root.
      root.xpath(".//*").each { |element| remove_redundant_namespaces(element) }
      root
    end

    def remove_redundant_namespaces(element)
      inherited = element.parent.namespaces
      element.namespace_definitions.each do |namespace|
        name = namespace.prefix ? "xmlns:#{namespace.prefix}" : "xmlns"
        element.remove_namespace_definition(namespace.prefix) if inherited[name] == namespace.href
      end
    end

    def validate_sequence
      names = steps.map(&:name)
      unless !names.empty? && PROFILES.values.any? { |profile| profile.take(names.length) == names }
        raise ArgumentError, "invalid step sequence: #{names.join(' -> ')}"
      end
    end

    def validate_scope(content)
      case profile
      when :svg_cleanup
        validate_svg_root(content)
      when :graft_then_svg_cleanup
        return if steps.first.options.fetch(:svg_host)

        raise ArgumentError, "execution-scope mismatch: graft_then_svg_cleanup requires an SVG host root"
      end
    rescue Leptris::XML::SyntaxError, Leptris::XML::ParseError => e
      raise ParsingError, "invalid content XML: #{e.message}"
    end

    def validate_svg_root(content)
      root = Leptris::XML.parse(content, strict: true).root
      return if root&.name == "svg"

      raise ArgumentError, "execution-scope mismatch: svg_cleanup requires an SVG root"
    end
  end
end
