# frozen_string_literal: true

require "leptris"

module Vectory
  # Replaces serialized root content while retaining the host document envelope.
  class SvgSubtreeGraft
    class << self
      # @return [String] the complete host document with processed root children
      def replace_root_content(host_content, processed_content)
        host = parse_document(host_content, "host_content")
        processed = parse_document(processed_content, "processed_content")
        children = processed.root.children.map { |node| copy_child(node) }
        host.root.children = children
        serialize_host(host, host_content)
      end

      private

      def serialize_host(host, content)
        # The document serializer moves DOCTYPE before leading comments/PIs.
        # Keep the original envelope and serialize only the retained host root.
        prefix = content.byteslice(0, host.root.byte_offset)
        suffix = content[/(?:[ \t\r\n]|<!--(?:(?!-->).)*-->|<\?(?:(?!\?>).)*\?>)*\z/m]
        "#{prefix}#{host.root.to_xml}#{suffix}"
      end

      def parse_document(content, label)
        document = Leptris::XML.parse(content, strict: true)
        raise ParsingError, "#{label} has no document root" unless document.root

        document
      rescue Leptris::XML::ParseError => e
        # Strict parsing also rejects valid rootless XML. Appending a root
        # validates that envelope without allowing repaired malformed input.
        begin
          Leptris::XML.parse("#{content}<vectory-root-check/>", strict: true)
        rescue Leptris::XML::ParseError
          raise ParsingError, "invalid #{label} XML: #{e.message}"
        end
        raise ParsingError, "#{label} has no document root"
      end

      def copy_child(node)
        copy = node.dup
        return copy unless node.element?

        # dup does not retain declarations inherited from the processed root.
        # Seal the copied subtree's scope before adopting it into the host.
        namespaces = { "xmlns" => "" }.merge(node.namespaces)
        declared = copy.namespace_definitions.map(&:prefix)
        namespaces.each do |name, uri|
          prefix = name == "xmlns" ? nil : name.delete_prefix("xmlns:")
          next if declared.include?(prefix)

          copy.add_namespace_definition(prefix, uri)
        end
        copy
      end
    end
  end
end
