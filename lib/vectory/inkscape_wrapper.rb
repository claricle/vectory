# frozen_string_literal: true

require "singleton"
require "tmpdir"
require "ukiryu"

module Vectory
  # InkscapeWrapper using Ukiryu for platform-adaptive command execution
  #
  # This class provides backward compatibility with the original InkscapeWrapper
  # while using Ukiryu under the hood for shell detection, escaping, and execution.
  class InkscapeWrapper
    include Singleton

    class << self
      def convert(content:, input_format:, output_format:, output_class:,
plain: false)
        instance.convert(
          content: content,
          input_format: input_format,
          output_format: output_format,
          output_class: output_class,
          plain: plain,
        )
      end
    end

    def convert(content:, input_format:, output_format:, output_class:,
plain: false)
      with_temp_files(content, input_format, output_format) do |input_path, output_path|
        # Get the tool
        tool = get_inkscape_tool

        # Build parameters
        params = build_export_params(input_path, output_path, output_format, plain)

        # Execute export command
        result = tool.execute(:export,
                              execution_timeout: Configuration.instance.timeout,
                              **params)

        raise_conversion_error(result) unless result.success?

        # Check if output file exists at specified path
        unless File.exist?(output_path)
          # Raise error with stderr details if output file not found
          # This handles cases where Inkscape returns exit code 0 but fails to create output
          raise Vectory::ConversionError,
                "Output file not found. " \
                "Expected: #{output_path}\n" \
                "Command: '#{result.command}',\n" \
                "Exit status: '#{result.status}',\n" \
                "stdout: '#{result.stdout.strip}',\n" \
                "stderr: '#{result.stderr.strip}'."
        end

        output_class.from_path(output_path)
      end
    end

    def height(content, format)
      query_integer(content, format, :height)
    end

    def width(content, format)
      query_integer(content, format, :width)
    end

    private

    # Get the Inkscape tool from Ukiryu
    def get_inkscape_tool
      Ukiryu::Tool.get("inkscape")
    rescue Ukiryu::Errors::ToolNotFoundError => e
      # Tool not found - raise the original InkscapeNotFoundError
      raise InkscapeNotFoundError, "Inkscape not available: #{e.message}"
    end

    # Build export parameters for Ukiryu
    def build_export_params(input_path, output_path, output_format, plain)
      params = {
        inputs: [input_path],
        output: output_path,
      }

      # Add format if specified (different from output extension)
      # Inkscape can detect format from output extension in modern versions
      # But we can be explicit
      params[:format] = output_format.to_sym if output_format

      # Add plain SVG flag
      params[:plain] = true if plain && output_format == :svg

      # Note: PDF import via Inkscape on macOS may have compatibility issues
      # The pages option can specify which page to import, but may not work on all platforms

      params
    end

    # Find the output file (Inkscape may create it with different name)
    def find_output(source_path, output_extension)
      basenames = [File.basename(source_path, ".*"),
                   File.basename(source_path)]

      paths = basenames.map do |basename|
        "#{File.join(File.dirname(source_path), basename)}.#{output_extension}"
      end

      paths.find { |p| File.exist?(p) }
    end

    # Raise conversion error with details
    def raise_conversion_error(result)
      raise Vectory::ConversionError,
            "Could not convert with Inkscape. " \
            "Command: '#{result.command}',\n" \
            "Exit status: '#{result.status}',\n" \
            "stdout: '#{result.stdout.strip}',\n" \
            "stderr: '#{result.stderr.strip}'."
    end

    # Query integer value from Inkscape
    #
    # @param content [String] the file content
    # @param format [String] the file format
    # @param param_key [Symbol] the query parameter key (:width, :height, :x, :y)
    # @return [Integer] the query result as an integer
    def query_integer(content, format, param_key)
      query(content, format, param_key).to_f.round
    end

    # Query Inkscape for information
    #
    # @param content [String] the file content
    # @param format [String] the file format
    # @param param_key [Symbol] the query parameter key (:width, :height, :x, :y)
    # @return [String] the query result
    def query(content, format, param_key)
      tool = get_inkscape_tool
      raise InkscapeNotFoundError, "Inkscape not available" unless tool

      with_temp_file(content, format) do |path|
        params = { input: path, param_key => true }

        result = tool.execute(:query,
                              execution_timeout: Configuration.instance.timeout,
                              **params)
        raise_query_error(result) if result.stdout.empty?

        result.stdout
      end
    end

    # Create temp file with content
    def with_temp_file(content, extension)
      Dir.mktmpdir do |dir|
        path = File.join(dir, "image.#{extension}")
        File.binwrite(path, content)

        yield path
      end
    end

    # Create temp files for input and output
    def with_temp_files(content, input_format, output_format)
      Dir.mktmpdir do |dir|
        input_path = File.join(dir, "image.#{input_format}")
        output_path = File.join(dir, "image.#{output_format}")
        File.binwrite(input_path, content)

        begin
          yield input_path, output_path
        ensure
          # On Windows, aggressively clean up temp files to avoid ENOTEMPTY errors
          # caused by Inkscape leaving behind lock files or hanging processes
          cleanup_temp_dir(dir) if Platform.windows?
        end
      end
    end

    # Aggressively clean up temp directory on Windows
    # Handles cases where Inkscape leaves behind files or processes
    def cleanup_temp_dir(dir)
      # Give processes a moment to release file handles
      sleep(0.2)

      # Try to remove all files in the directory
      Dir.glob(File.join(dir, "**", "*")).reverse_each do |file|
        File.delete(file) if File.file?(file)
      rescue Errno::EACCES, Errno::ENOENT
        # File may be locked or already deleted, ignore
      end

      # Try to remove subdirectories
      Dir.glob(File.join(dir, "**", "*")).reverse_each do |path|
        Dir.rmdir(path) if File.directory?(path)
      rescue Errno::EACCES, Errno::ENOENT, Errno::ENOTEMPTY
        # Directory may be locked or not empty, ignore
      end
    rescue StandardError
      # Best effort cleanup, don't raise
    end

    # Raise query error with details
    def raise_query_error(result)
      raise Vectory::InkscapeQueryError,
            "Could not query with Inkscape. " \
            "Command: '#{result.command}',\n" \
            "Exit status: '#{result.status}',\n" \
            "stdout: '#{result.stdout.strip}',\n" \
            "stderr: '#{result.stderr.strip}'."
    end

    # Format paths for command execution on current platform
    # Handles Windows backslash conversion and quoting for paths with spaces
    def external_path(path)
      return path unless path
      return path unless Platform.windows?

      # Convert forward slashes to backslashes
      path.gsub!(%r{/}, "\\")
      # Quote paths with spaces
      path[/\s/] ? "\"#{path}\"" : path
    end
  end
end
