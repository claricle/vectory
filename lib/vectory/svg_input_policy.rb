# frozen_string_literal: true

module Vectory
  # Inspect raw bytes before parsing can discard a DTD or expand an entity.
  module SvgInputPolicy
    DOCTYPE = /\A<!DOCTYPE(?:\s|\[|>)/

    def self.validate!(content)
      return unless content.is_a?(String)

      # XML permits UTF-16/32 declarations; ASCII token bytes are NUL padded.
      bytes = content.b.delete("\0")
      cursor = 0
      while (opening = bytes.index("<", cursor))
        cursor = token_end(bytes, opening)
      end
    end

    # Literal text inside these tokens is not markup.
    OPAQUE = { "<!--" => "-->", "<![CDATA[" => "]]>", "<?" => "?>" }.freeze

    def self.token_end(bytes, opening)
      return doctype_end(bytes, opening + 9) if bytes[opening, 10].match?(DOCTYPE)

      start, delimiter = OPAQUE.find { |token, _end| bytes[opening, token.bytesize] == token }
      start ? closing(bytes, delimiter, opening + start.bytesize) : opening + 1
    end

    # A DOCTYPE without an internal subset declares no entities, and the XML
    # parser never loads its external identifier, so only `[` is refused.
    def self.doctype_end(bytes, cursor)
      while cursor < bytes.bytesize
        case bytes[cursor]
        when ">" then return cursor + 1
        when "[" then raise ForbiddenExternalReferenceError, "XML DOCTYPE internal subsets are forbidden"
        when '"', "'" then cursor = closing(bytes, bytes[cursor], cursor + 1)
        else cursor += 1
        end
      end
      cursor
    end

    def self.closing(bytes, delimiter, start)
      index = bytes.index(delimiter, start)
      # An unterminated token consumes the rest; the XML parser diagnoses it.
      index ? index + delimiter.bytesize : bytes.bytesize
    end
    private_class_method :token_end, :doctype_end, :closing
  end
end
