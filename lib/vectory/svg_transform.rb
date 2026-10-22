# frozen_string_literal: true

require "leptris"

module Vectory
  # An immutable, named operation at a serialized XML boundary.
  class SvgTransform
    attr_reader :name, :options

    class << self
      def graft_into(host_content:)
        host = host_content.dup.freeze
        root = Leptris::XML.parse(host, strict: true).root
        raise ParsingError, "host_content has no document root" unless root

        new(:graft_into, { host_content: host, svg_host: root.name == "svg" })
      rescue Leptris::XML::SyntaxError, Leptris::XML::ParseError => e
        raise ParsingError, "invalid host_content XML: #{e.message}"
      end

      def disambiguate(index:, duplicate_ids:)
        new(:disambiguate, { index: index, duplicate_ids: duplicate_ids })
      end

      def suffix_classes(index:)
        new(:suffix_classes, { index: index })
      end

      def bind_namespaces(bind: {}, unbind: [], remap: {})
        new(:bind_namespaces, { bind: bind, unbind: unbind, remap: remap })
      end

      private

      def snapshot(value)
        case value
        when Hash
          value.to_h { |key, item| [snapshot(key), snapshot(item)] }.freeze
        when Array
          value.map { |item| snapshot(item) }.freeze
        else
          value.frozen? ? value : value.dup.freeze
        end
      end
    end

    def initialize(name, options)
      @name = name
      @options = self.class.send(:snapshot, options)
      freeze
    end
    private_class_method :new

    def >>(other)
      SvgTransformPipeline.new([self]) >> other
    end

    def call(content)
      SvgTransformPipeline.new([self]).call(content)
    end

    # Pipeline execution owns ordering and scope checks.
    def apply(content)
      case name
      when :graft_into
        SvgSubtreeGraft.replace_root_content(options.fetch(:host_content), content)
      when :disambiguate
        SvgIdDisambiguator.rewrite(content, suffix, options.fetch(:duplicate_ids))
      when :suffix_classes
        SvgClassSuffixer.rewrite(content, suffix)
      when :bind_namespaces
        SvgNamespaceBinding.rewrite(content, **options)
      end
    end

    private

    def suffix
      "_inject_#{options.fetch(:index)}"
    end
  end
end
