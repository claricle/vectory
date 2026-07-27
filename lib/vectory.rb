# frozen_string_literal: true

# External dependencies
require "logger"

# Define base error class and additional error classes
# (used in class bodies like cli.rb, so can't be autoloaded)
module Vectory
  class Error < StandardError; end

  class SystemCallError < Error; end

  class NotImplementedError < Error; end

  class NotWrittenToDiskError < Error; end

  class ParsingError < Error; end
end

require_relative "vectory/errors"
require_relative "vectory/version"

# Lazy load: all other internal Vectory dependencies via autoload
module Vectory
  # Core utilities
  autoload :Utils, "vectory/utils"

  # Format classes
  autoload :Configuration, "vectory/configuration"
  autoload :Image, "vectory/image"
  autoload :ImageResize, "vectory/image_resize"
  autoload :Datauri, "vectory/datauri"
  autoload :Vector, "vectory/vector"
  autoload :Eps, "vectory/eps"
  autoload :Ps, "vectory/ps"
  autoload :Emf, "vectory/emf"
  autoload :Svg, "vectory/svg"
  autoload :SvgMapping, "vectory/svg_mapping"
  autoload :SvgDocument, "vectory/svg_document"
  autoload :FileMagic, "vectory/file_magic"
end

# Define additional module methods
module Vectory
  def self.ui
    @ui ||= Logger.new($stdout).tap do |logger|
      logger.level = ENV["VECTORY_LOG"] || Logger::WARN
      logger.formatter = proc { |_severity, _datetime, _progname, msg|
        "#{msg}\n"
      }
    end
  end

  def self.root_path
    Pathname.new(File.dirname(__dir__))
  end

  def self.convert(image, format)
    image.convert(format)
  end

  def self.image_resize(img, path, maxheight, maxwidth)
    Vectory::ImageResize.new.call(img, path, maxheight, maxwidth)
  end
end
