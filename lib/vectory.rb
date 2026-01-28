# frozen_string_literal: true

require "logger"
require_relative "vectory/version"
require_relative "vectory/utils"

module Vectory
  class Error < StandardError; end
end

require_relative "vectory/errors"
require_relative "vectory/platform"
require_relative "vectory/system_command"

# Configure SystemCommand to use Vectory::Platform for enhanced platform detection
# This allows SystemCommand to benefit from Platform's additional methods
# (macos?, linux?, command_available?, etc.) while remaining reusable by other libraries
Vectory::SystemCommand.platform_detector = Vectory::Platform

require_relative "vectory/configuration"
require_relative "vectory/conversion"
require_relative "vectory/image"
require_relative "vectory/image_resize"
require_relative "vectory/datauri"
require_relative "vectory/vector"
require_relative "vectory/ghostscript_wrapper"
require_relative "vectory/pdf"
require_relative "vectory/eps"
require_relative "vectory/ps"
require_relative "vectory/emf"
require_relative "vectory/svg"
require_relative "vectory/svg_mapping"

module Vectory
  class SystemCallError < Error; end

  class NotImplementedError < Error; end

  class NotWrittenToDiskError < Error; end

  class ParsingError < Error; end

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
