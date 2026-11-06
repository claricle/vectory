# frozen_string_literal: true

require "image_size"

module Vectory
  class Png < Image
    def self.default_extension
      "png"
    end

    def self.mimetype
      "image/png"
    end

    attr_reader :warnings

    def initialize(content, initial_path = nil, warnings: [])
      super(content.b, initial_path)
      @warnings = warnings.dup.freeze
    end

    def mime
      self.class.mimetype
    end

    def width
      dimensions.width
    end

    def height
      dimensions.height
    end

    private

    def dimensions
      @dimensions ||= ImageSize.new(content)
    end
  end
end
