# frozen_string_literal: true

require "leptris"

module Vectory
  # Replaces serialized root content while retaining the host document envelope.
  class SvgSubtreeGraft
    ENCODING_SIGNATURES = {
      "\x00\x00\xFE\xFF".b => "UTF-32", "\xFF\xFE\x00\x00".b => "UTF-32",
      "\xFE\xFF".b => "UTF-16", "\xFF\xFE".b => "UTF-16",
      "\x00\x00\x00<".b => "UTF-32BE", "<\x00\x00\x00".b => "UTF-32LE",
      "\x00<\x00?".b => "UTF-16BE", "<\x00?\x00".b => "UTF-16LE"
    }.freeze
    private_constant :ENCODING_SIGNATURES

    class << self
      # @return [String] the complete host document with processed root children
      def replace_root_content(host_content, processed_content)
        host_content = utf8_content(host_content, "host_content")
        processed_content = utf8_content(processed_content, "processed_content")
        host = parse_document(host_content, "host_content")
        processed = parse_document(processed_content, "processed_content")
        # Native node copying/adoption can drop interleaved namespaced attributes.
        # Reparse serialized children instead of crossing that copy boundary.
        children = processed.root.children.map { |node| serialize_child(node, host.root.namespaces) }
        host.root.children = children
        serialize_host(host, host_content)
      end

      # @api private
      # Leptris consumes UTF-8 bytes without decoding XML encoding declarations.
      # Normalize a working copy; captured pipeline options retain the original.
      def utf8_content(content, label)
        encoding = source_encoding(content)
        utf8 = content.dup.force_encoding(encoding).encode(Encoding::UTF_8)
        return utf8 if Encoding.find(encoding) == Encoding::UTF_8

        utf8.sub(/\A(<\?xml[ \t\r\n][^?]*\bencoding\s*=\s*)(["'])[^"']+\2/) do
          "#{Regexp.last_match(1)}#{Regexp.last_match(2)}UTF-8#{Regexp.last_match(2)}"
        end
      rescue EncodingError, ArgumentError => e
        raise ParsingError, "invalid #{label} XML: #{e.message}"
      end

      private

      def source_encoding(content)
        bytes = content.b
        signature = ENCODING_SIGNATURES.find { |prefix, _| bytes.start_with?(prefix) }
        return signature.last if signature

        bytes[/\A(?:\xEF\xBB\xBF)?<\?xml[ \t\r\n][^?]*\bencoding\s*=\s*(["'])([^"']+)\1/n, 2] || "UTF-8"
      end

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

      def serialize_child(node, host_namespaces)
        seal_namespaces(node, host_namespaces) if node.element?
        node.to_xml
      end

      def seal_namespaces(node, host_namespaces)
        # Retain inherited bindings only where the host cannot supply them.
        namespaces = { "xmlns" => "" }.merge(node.namespaces)
        declared = node.namespace_definitions.map(&:prefix)
        namespaces.each do |name, uri|
          prefix = name == "xmlns" ? nil : name.delete_prefix("xmlns:")
          next if declared.include?(prefix) || host_namespaces[name] == uri

          node.add_namespace_definition(prefix, uri)
        end
      end
    end
  end
end
