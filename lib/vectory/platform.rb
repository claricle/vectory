# frozen_string_literal: true

module Vectory
  # Platform abstraction for centralized OS-specific behavior
  #
  # This class provides a single source of truth for platform detection
  # and platform-specific path handling, eliminating duplicated logic
  # across InkscapeWrapper, GhostscriptWrapper, and other classes.
  #
  # @example Check if running on Windows
  #   Vectory::Platform.windows? # => true or false
  #
  # @example Format a path for execution on the current platform
  #   Vectory::Platform.path_for_execution("C:/Program Files/Inkscape/inkscape.exe")
  #   # On Windows: "C:\\Program Files\\Inkscape\\inkscape.exe"
  #   # On Unix:   "C:/Program Files/Inkscape/inkscape.exe"
  class Platform
    class << self
      # Detect if running on Windows
      #
      # @return [Boolean] true if on Windows platform
      def windows?
        Gem.win_platform?
      end

      # Detect if running on macOS
      #
      # @return [Boolean] true if on macOS platform
      def macos?
        RbConfig::CONFIG['host_os'] =~ /darwin/
      end

      # Detect if running on Linux
      #
      # @return [Boolean] true if on Linux platform
      def linux?
        RbConfig::CONFIG['host_os'] =~ /linux/
      end

      # Format a file path for execution on the current platform
      #
      # On Windows, converts forward slashes to backslashes and quotes paths with spaces.
      # On Unix-like systems, returns the path unchanged.
      #
      # @param path [String] the file path to format
      # @return [String] platform-formatted path
      def path_for_execution(path)
        return path unless path

        formatted_path = windows? ? path.gsub('/', '\\') : path

        # Quote paths with spaces to prevent shell parsing issues
        formatted_path[/\s/] ? "\"#{formatted_path}\"" : formatted_path
      end

      # Get the PATH environment variable as an array
      #
      # Handles different PATH separators on Windows (;) vs Unix (:)
      #
      # @return [Array<String>] array of directory paths
      def executable_search_paths
        @executable_search_paths ||= begin
          path_sep = windows? ? ';' : ':'
          (ENV['PATH'] || '').split(path_sep)
        end
      end

      # Check if a command is available in the system PATH
      #
      # @param command [String] the command to check
      # @return [Boolean] true if command is found in PATH
      def command_available?(command)
        executable_search_paths.any? do |dir|
          executable_path = File.join(dir, command)
          File.executable?(executable_path) && !File.directory?(executable_path)
        end
      end

      # Get the appropriate shell command extension for the platform
      #
      # @return [String, nil] ".exe" on Windows, nil on Unix
      def command_extension
        windows? ? '.exe' : nil
      end

      # Get the default shell for the platform
      #
      # @return [String] shell command (e.g., "cmd.exe" on Windows, "sh" on Unix)
      def default_shell
        if windows?
          'cmd.exe'
        else
          'sh'
        end
      end

      # Reset cached values (primarily for testing)
      #
      # @api private
      def reset_cache
        @executable_search_paths = nil
      end
    end
  end
end
