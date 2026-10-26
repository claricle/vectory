# frozen_string_literal: true

module Vectory
  # Converts command-line PNG options into the renderer's public values.
  class PngOptions
    NAMES = %w[font fallback_font width height scale background].freeze

    def self.validate_format!(options)
      return if options[:format] == "png"
      return unless NAMES.any? { |name| options.key?(name) }

      raise ConversionError, "Font and render-profile options require PNG output"
    end

    def initialize(options)
      @options = options
    end

    def keywords
      PngRenderer.with_errors do
        { profile: profile, fonts: fonts, fallback_fonts: fallback_fonts }
      end
    end

    private

    def profile
      %w[width height scale background].each_with_object({}) do |name, values|
        next unless @options.key?(name)

        values[name.to_sym] = profile_value(name, @options[name])
      end
    end

    def profile_value(name, value)
      case name
      when "width", "height" then Integer(value, 10)
      else value
      end
    rescue ArgumentError, TypeError
      raise RenderProfileError, "Invalid PNG #{name}: #{value}"
    end

    def fonts
      Array(@options[:font]).each_with_object({}) do |assignment, families|
        family, face = font_face(assignment)
        (families[family] ||= []) << face
      end
    end

    def fallback_fonts
      Array(@options[:fallback_font]).map do |assignment|
        _family, face = font_face(assignment)
        face
      end
    end

    def font_face(assignment)
      family, path = assignment.split("=", 2)
      if family.nil? || family.strip.empty? || path.nil? || path.empty?
        raise FontError, "Font option must be FAMILY=PATH"
      end

      [family, Svgpng::FontFace.new(bytes: File.binread(path), source_id: path, family: family)]
    rescue ::SystemCallError, IOError => e
      raise Vectory::SystemCallError, "Cannot read font: #{e.message}"
    end
  end
end
