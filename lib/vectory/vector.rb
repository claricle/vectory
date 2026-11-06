# frozen_string_literal: true

module Vectory
  class Vector < Image
    def self.from_datauri(uri)
      Datauri.new(uri).to_vector
    end

    def self.default_extension
      raise Vectory::NotImplementedError,
            "#default_extension should be implemented in a subclass."
    end

    def self.mimetype
      raise Vectory::NotImplementedError,
            "#mimetype should be implemented in a subclass."
    end

    def mime
      self.class.mimetype
    end

    def height
      raise Vectory::NotImplementedError,
            "#height should be implemented in a subclass when no intrinsic " \
            "dimension source is available."
    end

    def width
      raise Vectory::NotImplementedError,
            "#width should be implemented in a subclass when no intrinsic " \
            "dimension source is available."
    end

    def to_uri
      Datauri.from_vector(self)
    end
  end
end
