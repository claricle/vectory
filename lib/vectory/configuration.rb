# frozen_string_literal: true

module Vectory
  # Configuration for Vectory
  #
  # Provides centralized configuration for tool paths, timeouts, caching, etc.
  # Can be loaded from environment variables or a configuration file.
  #
  # @example Set custom Inkscape path
  #   Vectory::Configuration.instance.inkscape_path = "/path/to/inkscape"
  #
  # @example Load from environment variables
  #   # Set VECTORY_INKSCAPE_PATH environment variable
  #   Vectory::Configuration.load_from_environment
  class Configuration
    # Default timeout for external tool execution (seconds)
    DEFAULT_TIMEOUT = 120

    # Default cache TTL for memoized values (seconds)
    DEFAULT_CACHE_TTL = 300

    # Default temporary directory
    DEFAULT_TEMP_DIR = nil # Use system default

    attr_accessor :inkscape_path, :ghostscript_path, :timeout, :cache_ttl,
                  :cache_enabled, :temp_dir, :verbose_logging

    # Get the singleton instance
    #
    # @return [Vectory::Configuration] the configuration instance
    def self.instance
      @instance ||= new
    end

    # Reset the configuration to defaults
    #
    # @api private
    def self.reset!
      @instance = new
    end

    # Initialize configuration with default values
    def initialize
      @timeout = DEFAULT_TIMEOUT
      @cache_ttl = DEFAULT_CACHE_TTL
      @cache_enabled = true
      @temp_dir = DEFAULT_TEMP_DIR
      @verbose_logging = false
      @inkscape_path = nil
      @ghostscript_path = nil
    end

    # Load configuration from environment variables
    #
    # Supported environment variables:
    # - VECTORY_INKSCAPE_PATH: Path to Inkscape executable
    # - VECTORY_GHOSTSCRIPT_PATH: Path to Ghostscript executable
    # - VECTORY_TIMEOUT: Timeout for external tools (default: 120)
    # - VECTORY_CACHE_TTL: Cache TTL in seconds (default: 300)
    # - VECTORY_CACHE_ENABLED: Enable/disable caching (default: true)
    # - VECTORY_TEMP_DIR: Temporary directory path
    # - VECTORY_VERBOSE: Enable verbose logging (default: false)
    #
    # @return [self] the configuration instance
    def self.load_from_environment
      config = instance

      config.inkscape_path = ENV["VECTORY_INKSCAPE_PATH"] if ENV["VECTORY_INKSCAPE_PATH"]
      config.ghostscript_path = ENV["VECTORY_GHOSTSCRIPT_PATH"] if ENV["VECTORY_GHOSTSCRIPT_PATH"]
      config.timeout = ENV["VECTORY_TIMEOUT"]&.to_i || config.timeout
      config.cache_ttl = ENV["VECTORY_CACHE_TTL"]&.to_i || config.cache_ttl
      config.cache_enabled = ENV["VECTORY_CACHE_ENABLED"] != "false"
      config.temp_dir = ENV["VECTORY_TEMP_DIR"] if ENV["VECTORY_TEMP_DIR"]
      config.verbose_logging = ENV["VECTORY_VERBOSE"] == "true"

      config
    end

    # Load configuration from a YAML file
    #
    # @param path [String] path to the YAML configuration file
    # @return [self] the configuration instance
    # @raise [Errno::ENOENT] if the file doesn't exist
    def self.load_from_file(path)
      require "yaml"
      config_data = YAML.load_file(path)

      config = instance
      config.inkscape_path = config_data["inkscape_path"] if config_data["inkscape_path"]
      config.ghostscript_path = config_data["ghostscript_path"] if config_data["ghostscript_path"]
      config.timeout = config_data["timeout"] || config.timeout
      config.cache_ttl = config_data["cache_ttl"] || config.cache_ttl
      config.cache_enabled = config_data.fetch("cache_enabled",
                                               config.cache_enabled)
      config.temp_dir = config_data["temp_dir"] || config.temp_dir
      config.verbose_logging = config_data.fetch("verbose_logging",
                                                 config.verbose_logging)

      config
    end

    # Export configuration as a hash
    #
    # @return [Hash] the configuration as a hash
    def to_h
      {
        inkscape_path: @inkscape_path,
        ghostscript_path: @ghostscript_path,
        timeout: @timeout,
        cache_ttl: @cache_ttl,
        cache_enabled: @cache_enabled,
        temp_dir: @temp_dir,
        verbose_logging: @verbose_logging,
      }
    end

    # Get the Inkscape path (custom or auto-detected)
    #
    # @return [String, nil] the configured Inkscape path or nil if not set
    def effective_inkscape_path
      @inkscape_path
    end

    # Get the Ghostscript path (custom or auto-detected)
    #
    # @return [String, nil] the configured Ghostscript path or nil if not set
    def effective_ghostscript_path
      @ghostscript_path
    end

    # Check if caching is enabled
    #
    # @return [Boolean] true if caching is enabled
    def caching_enabled?
      @cache_enabled
    end

    # Get the temporary directory
    #
    # @return [String] the temporary directory path
    def temporary_directory
      @temp_dir || Dir.tmpdir
    end

    # Check if verbose logging is enabled
    #
    # @return [Boolean] true if verbose logging is enabled
    def verbose_logging?
      @verbose_logging
    end

    # Validate the configuration
    #
    # @return [Boolean] true if configuration is valid
    # @raise [ArgumentError] if configuration is invalid
    def validate!
      errors = []

      if @timeout && @timeout <= 0
        errors << "timeout must be positive, got: #{@timeout}"
      end

      if @cache_ttl&.negative?
        errors << "cache_ttl must be non-negative, got: #{@cache_ttl}"
      end

      if @temp_dir && !File.directory?(@temp_dir)
        errors << "temp_dir does not exist: #{@temp_dir}"
      end

      return true if errors.empty?

      raise ArgumentError,
            "Invalid configuration:\n  - #{errors.join("\n  - ")}"
    end
  end
end
