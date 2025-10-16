require "base64"

module Vectory
  class Datauri < Image
    REGEX = %r{^
      data:
      (?<mimetype>[^;]+);
      (?:charset=[^;]+;)?
      base64,
      (?<data>.+)
    $}x

    def self.from_vector(vector)
      mimetype = vector.class.mimetype
      content = vector.content

      # Normalize line endings for text-based types before encoding
      # Apply to PS, EPS, and SVG. EMF is binary, so skip it.
      if ["application/postscript", "image/svg+xml"].include?(vector.mime)
        content = content.gsub("\r\n", "\n")
      end

      data = Base64.strict_encode64(content)

      new("data:#{mimetype};base64,#{data}")
    end

    def mime
      match = parse_datauri(@content)
      match[:mimetype]
    end

    def height
      to_vector.height
    end

    def width
      to_vector.width
    end

    def to_vector
      match = parse_datauri(@content)
      content = Base64.strict_decode64(match[:data])
      image_class = detect_image_class(match[:mimetype])

      image_class.from_content(content)
    end

    private

    def parse_datauri(uri)
      match = REGEX.match(uri)
      return match if match

      raise ConversionError, "Could not parse datauri: '#{uri.slice(0, 30)}'."
    end

    def detect_image_class(image_type)
      case image_type
      when Eps.mimetype then return Eps
      when *Emf.all_mimetypes then return Emf
      when Svg.mimetype then return Svg
      end

      raise ConversionError, "Could not detect image type '#{image_type}'."
    end
  end
end
