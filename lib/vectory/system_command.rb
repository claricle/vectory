# frozen_string_literal: true

module Vectory
  # SystemCommand provides cross-platform utilities for command execution
  #
  # This module centralizes platform-specific command execution concerns,
  # including path formatting, environment setup, and executable discovery.
  #
  # == Platform Detection
  #
  # SystemCommand uses a pluggable platform detector for OS-specific behavior.
  # By default, it uses a built-in detector based on Ruby's Gem.win_platform?
  # You can configure a custom detector:
  #
  #   # Use a custom platform detector
  #   SystemCommand.platform_detector = MyPlatformDetector
  #
  #   # The detector must respond to:
  #   # - windows? => Boolean
  #   # - executable_search_paths => Array<String>
  #
  # == Usage by Other Libraries
  #
  # To use SystemCommand in your own library:
  #
  #   require "vectory/system_command"
  #
  #   # Use with default platform detector
  #   SystemCommand.format_path("C:/Program Files/tool.exe")
  #   SystemCommand.headless_environment
  #   SystemCommand.find_executable("tool")
  #
  #   # Or configure your own platform detector
  #   SystemCommand.platform_detector = MyPlatform::Detector
  #
  # @example Format a path for Windows command execution
  #   Vectory::SystemCommand.format_path("C:/Program Files/inkscape.exe")
  #   # => "\"C:\\Program Files\\inkscape.exe\""
  #
  # @example Get environment for headless operation
  #   Vectory::SystemCommand.headless_environment
  #   # On Unix:   => { "DISPLAY" => "" }
  #   # On Windows: => {}
  #
  # @example Find an executable in PATH
  #   Vectory::SystemCommand.find_executable("gs")
  #   # => "/usr/bin/gs" or nil
  module SystemCommand
    class << self
      # Get or set the platform detector
      #
      # The platform detector must respond to:
      # - windows? => Boolean
      # - executable_search_paths => Array<String>
      #
      # @return [#windows?, #executable_search_paths] the current platform detector
      attr_accessor :platform_detector

      # Format a file path for safe command execution on the current platform
      #
      # On Windows, converts forward slashes to backslashes and quotes paths
      # containing spaces to prevent shell parsing issues.
      # On Unix-like systems, returns the path unchanged.
      #
      # @param path [String] the file path to format
      # @return [String] platform-formatted path suitable for command execution
      #
      # @example Format a Windows path with spaces
      #   format_path("C:/Program Files/Inkscape/inkscape.exe")
      #   # => "\"C:\\Program Files\\Inkscape\\inkscape.exe\""
      #
      # @example Format a Unix path (unchanged)
      #   format_path("/usr/bin/inkscape")
      #   # => "/usr/bin/inkscape"
      def format_path(path)
        return path unless path
        return path unless detector.windows?

        # Convert forward slashes to backslashes
        formatted = path.gsub(%r{/}, "\\")
        # Quote paths with spaces to prevent shell parsing issues
        formatted[/\s/] ? "\"#{formatted}\"" : formatted
      end

      # Get environment variables for headless operation
      #
      # On Unix-like systems (macOS, Linux), disables DISPLAY to prevent
      # X11/GDK warnings when GUI tools are run without a display server.
      # On Windows, returns an empty hash as there's no DISPLAY variable.
      #
      # @return [Hash] environment variables for headless execution
      #
      # @example On Unix-like systems
      #   headless_environment
      #   # => { "DISPLAY" => "" }
      #
      # @example On Windows
      #   headless_environment
      #   # => {}
      def headless_environment
        detector.windows? ? {} : { "DISPLAY" => "" }
      end

      # Find an executable in the system PATH
      #
      # Searches the PATH environment variable for the given command,
      # handling PATHEXT extensions on Windows for executable discovery.
      #
      # @param command [String] the command or executable name to find
      # @return [String, nil] the full path to the executable, or nil if not found
      #
      # @example Find Inkscape on Unix
      #   find_executable("inkscape")
      #   # => "/usr/bin/inkscape"
      #
      # @example Find Ghostscript on Windows
      #   find_executable("gswin64c")
      #   # => "C:\\Program Files\\gs\\10.01.2\\bin\\gswin64c.exe"
      #
      # @example Command not found
      #   find_executable("nonexistent")
      #   # => nil
      def find_executable(command)
        # Try with PATHEXT extensions (Windows executables)
        exts = ENV["PATHEXT"] ? ENV["PATHEXT"].split(";") : [""]

        detector.executable_search_paths.each do |dir|
          exts.each do |ext|
            exe = File.join(dir, "#{command}#{ext}")
            return exe if File.executable?(exe) && !File.directory?(exe)
          end
        end

        nil
      end

      private

      # Get the current platform detector, defaulting to built-in detector
      #
      # @return [#windows?, #executable_search_paths] the platform detector
      def detector
        @platform_detector ||= DefaultPlatformDetector
      end
    end

    # Built-in default platform detector using standard Ruby
    #
    # This detector uses Gem.win_platform? for Windows detection and
    # standard ENV["PATH"] parsing for executable search paths.
    #
    # @api private
    module DefaultPlatformDetector
      class << self
        # Detect if running on Windows
        #
        # @return [Boolean] true if on Windows platform
        def windows?
          Gem.win_platform?
        end

        # Get the PATH environment variable as an array
        #
        # Handles different PATH separators on Windows (;) vs Unix (:)
        #
        # @return [Array<String>] array of directory paths
        def executable_search_paths
          @executable_search_paths ||= begin
            path_sep = windows? ? ";" : ":"
            (ENV["PATH"] || "").split(path_sep)
          end
        end

        # Reset cached paths (primarily for testing)
        #
        # @api private
        def reset_cache
          @executable_search_paths = nil
        end
      end
    end
  end
end
