# frozen_string_literal: true

require "tmpdir"

module Vectory
  class Image
    class << self
      def from_path(path)
        new(File.read(path, mode: "rb"), path)
      end

      def from_content(content)
        new(content)
      end
    end

    attr_reader :content, :initial_path

    def initialize(content = nil, initial_path = nil)
      @content = content
      @initial_path = initial_path
    end

    def size
      content.bytesize
    end

    def file_size
      raise(NotWrittenToDiskError) unless @path

      File.size(@path)
    end

    def write(path = nil)
      target_path = path || @path || tmp_path
      File.binwrite(target_path, content)
      @path = File.expand_path(target_path)

      self
    end

    def path
      @path || raise(NotWrittenToDiskError)
    end

    private

    attr_writer :content

    def tmp_path
      dir = Dir.mktmpdir
      filename = "image.#{self.class.default_extension}"
      File.join(dir, filename)
    end
  end
end
