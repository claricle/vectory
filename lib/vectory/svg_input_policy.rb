# frozen_string_literal: true

module Vectory
  # Inspect raw bytes before parsing can discard a DTD or expand an entity.
  module SvgInputPolicy
    def self.validate!(content)
      return unless content.is_a?(String)

      # XML permits UTF-16/32 declarations; ASCII token bytes are NUL padded.
      bytes = content.b.delete("\0")
      cursor = 0
      while (opening = bytes.index("<", cursor))
        cursor = token_end(bytes, opening)
      end
    end

    def self.token_end(bytes, opening)
      return closing(bytes, "-->", opening + 4) if bytes[opening, 4] == "<!--"
      return closing(bytes, "]]>", opening + 9) if bytes[opening, 9] == "<![CDATA["
      return closing(bytes, "?>", opening + 2) if bytes[opening, 2] == "<?"
      if bytes[opening, 10].match?(/\A<!DOCTYPE(?:\s|\[|>)/)
        raise ForbiddenExternalReferenceError, "XML DOCTYPE declarations are forbidden"
      end

      opening + 1
    end

    def self.closing(bytes, delimiter, start)
      index = bytes.index(delimiter, start)
      # An unterminated token consumes the rest; the XML parser diagnoses it.
      index ? index + delimiter.bytesize : bytes.bytesize
    end
    private_class_method :token_end, :closing
  end
end
