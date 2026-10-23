# frozen_string_literal: true

require "leptris"

module Vectory
  # Declaration changes on one serialized SVG. Remaps use namespace URIs,
  # rather than lexical source prefixes, so shadows and repeats are unambiguous.
  # Existing aliases remain unless explicitly requested in +unbind+.
  class SvgNamespaceBinding
    def self.rewrite(content, bind: {}, unbind: [], remap: {})
      new(content, bind, unbind, remap).rewrite
    end

    def initialize(content, bind, unbind, remap)
      unless bind.is_a?(Hash) && unbind.is_a?(Array) && remap.is_a?(Hash)
        raise ArgumentError, "invalid namespace request: expected maps and an array"
      end

      normalize_options(bind, unbind, remap)
      @document = parse(content)
      @elements = @document.xpath("//*")
      validate_names
    end

    def rewrite
      targets = requested_targets
      targets.each { |prefix, uri| preflight_target(prefix, uri) }
      preflight_unbind
      targets.each { |prefix, uri| bind_target(prefix, uri) }
      @elements.each { |element| remap_element(element) }
      unbind_namespaces
      @document.to_xml
    end

    private

    def normalize_options(bind, unbind, remap)
      @bind = normalize_bindings(bind)
      @unbind = unbind.map { |prefix| normalize_prefix(prefix) }.uniq
      @remap = remap.to_h { |uri, prefix| [validate_uri(uri), normalize_prefix(prefix)] }
    end

    def normalize_bindings(bind)
      bindings = {}
      bind.each do |prefix, uri|
        prefix = normalize_prefix(prefix)
        uri = validate_uri(uri)
        add_target(bindings, prefix, uri)
      end
      bindings
    end

    def requested_targets
      targets = @bind.dup
      @remap.each { |uri, prefix| add_target(targets, prefix, uri) }
      targets
    end

    def add_target(targets, prefix, uri)
      if targets.key?(prefix) && targets[prefix] != uri
        raise ArgumentError, "namespace conflict: incompatible requested bindings"
      end

      targets[prefix] = uri
    end

    def bind_target(prefix, uri)
      @document.root.add_namespace_definition(prefix, uri) unless
        @document.root.namespaces[declaration_name(prefix)] == uri
    end

    def unbind_element(element)
      element.namespace_definitions.each do |namespace|
        element.remove_namespace_definition(namespace.prefix) if @unbind.include?(namespace.prefix)
      end
    end

    def unbind_namespaces
      # Descendants still need their inherited source bindings during remapping.
      @elements.each { |element| unbind_element(element) }
    end

    def normalize_prefix(prefix)
      return nil if prefix.nil? || prefix == ""
      if !prefix.is_a?(String) || !/\A[A-Za-z_][A-Za-z0-9_.-]*\z/.match?(prefix) ||
          prefix.downcase.start_with?("xml")
        raise ArgumentError, "invalid namespace request: invalid or reserved prefix"
      end

      prefix
    end

    def validate_uri(uri)
      unless uri.is_a?(String) && !uri.empty? && !/\s/.match?(uri) &&
          !["http://www.w3.org/XML/1998/namespace", "http://www.w3.org/2000/xmlns/"].include?(uri)
        raise ArgumentError, "invalid namespace request: expected nonempty namespace URI"
      end

      uri
    end

    def parse(content)
      raise ArgumentError, "invalid SVG: expected serialized string" unless content.is_a?(String)

      document = Leptris::XML.parse(content, strict: true)
      unless document.root&.name == "svg"
        raise ArgumentError, "invalid SVG: expected SVG root"
      end

      document
    rescue Leptris::XML::Error => e
      raise ArgumentError, "invalid SVG: #{e.message}"
    end

    def declaration_name(prefix)
      prefix ? "xmlns:#{prefix}" : "xmlns"
    end

    def validate_names
      @elements.each do |element|
        if element.name.include?(":") && !element.namespace&.href
          raise ArgumentError, "invalid SVG: unbound element prefix"
        end

        validate_attributes(element)
      end
    end

    def validate_attributes(element)
      element.attribute_nodes.each do |attr|
        if attr.name.include?(":") && !attr.namespace_uri
          raise ArgumentError, "invalid SVG: unbound attribute prefix"
        end
      end
    end

    def preflight_target(prefix, uri)
      preflight_declarations(prefix, uri)
      preflight_default(uri) unless prefix
    end

    def preflight_declarations(prefix, uri)
      @elements.each do |element|
        element.namespace_definitions.each do |namespace|
          if namespace.prefix == prefix && namespace.href != uri
            raise ArgumentError, "namespace conflict: target prefix already bound"
          end
        end
      end
    end

    def preflight_default(uri)
      if @elements.any? { |element| !element.namespace&.href }
        raise ArgumentError, "default namespace: would qualify unqualified elements"
      end
      return unless remapped_to_default?(uri)
      return unless attributes_use_uri?(uri)

      raise ArgumentError, "default namespace: attributes require an explicit prefix"
    end

    def remapped_to_default?(uri)
      @remap[uri].nil? && @remap.key?(uri)
    end

    def attributes_use_uri?(uri)
      @elements.any? { |element| element.attribute_nodes.any? { |attr| attr.namespace_uri == uri } }
    end

    def preflight_unbind
      @unbind.each do |prefix|
        if @bind.key?(prefix) || @remap.value?(prefix)
          raise ArgumentError, "namespace conflict: target also requested for unbind"
        end

        preflight_prefix_usage(prefix)
      end
    end

    def preflight_prefix_usage(prefix)
      @elements.each do |element|
        used = element_uses_prefix?(element, prefix) || attributes_use_prefix?(element, prefix)
        raise ArgumentError, "namespace in use: cannot remove requested prefix" if used
      end
    end

    def element_uses_prefix?(element, prefix)
      namespace = element.namespace
      namespace && namespace.prefix == prefix && !@remap.key?(namespace.href)
    end

    def attributes_use_prefix?(element, prefix)
      element.attribute_nodes.any? do |attr|
        attr.namespace_uri && attr.name.split(":")[0...-1].join(":") == prefix &&
          !@remap.key?(attr.namespace_uri)
      end
    end

    def remap_element(element)
      # Snapshot before changing names: native namespace resolution is lexical.
      namespace = element.namespace
      attributes = attribute_snapshot(element)
      if namespace && @remap.key?(namespace.href)
        element.name = qualified_name(@remap[namespace.href], element.name)
      end
      remap_attributes(element, attributes)
    end

    def attribute_snapshot(element)
      element.attribute_nodes.map do |attr|
        [attr, attr.namespace_uri, attr.name.split(":").last, attr.value]
      end
    end

    def remap_attributes(element, attributes)
      attributes.each do |attr, uri, local, value|
        next unless @remap.key?(uri)

        attr.remove
        element[qualified_name(@remap[uri], local)] = value
      end
    end

    def qualified_name(prefix, local)
      prefix ? "#{prefix}:#{local}" : local
    end
  end
end
