# frozen_string_literal: true

require "singleton"
require "tmpdir"
require_relative "system_call"
require_relative "platform"

module Vectory
  class InkscapeWrapper
    include Singleton

    def self.convert(content:, input_format:, output_format:, output_class:,
plain: false)
      instance.convert(
        content: content,
        input_format: input_format,
        output_format: output_format,
        output_class: output_class,
        plain: plain,
      )
    end

    def convert(content:, input_format:, output_format:, output_class:,
plain: false)
      with_temp_files(content, input_format,
                      output_format) do |input_path, output_path|
        exe = inkscape_path_or_raise_error
        exe = external_path(exe)
        input_path = external_path(input_path)
        output_path = external_path(output_path)

        cmd = build_command(exe, input_path, output_path, output_format, plain)
        # Pass environment to disable display on non-Windows systems
        env = headless_environment
        call = SystemCall.new(cmd, env: env).call

        actual_output = find_output(input_path, output_format)
        raise_conversion_error(call) unless actual_output

        output_class.from_path(actual_output)
      end
    end

    def height(content, format)
      query_integer(content, format, "--query-height")
    end

    def width(content, format)
      query_integer(content, format, "--query-width")
    end

    private

    def inkscape_path_or_raise_error
      inkscape_path or raise(InkscapeNotFoundError,
                             "Inkscape missing in PATH, unable to " \
                             "convert image. Aborting.")
    end

    def inkscape_path
      @inkscape_path ||= find_inkscape
    end

    def find_inkscape
      cmds.each do |cmd|
        extensions.each do |ext|
          Platform.executable_search_paths.each do |path|
            exe = File.join(path, "#{cmd}#{ext}")

            return exe if File.executable?(exe) && !File.directory?(exe)
          end
        end
      end

      nil
    end

    def cmds
      ["inkscapecom", "inkscape"]
    end

    def extensions
      ENV["PATHEXT"] ? ENV["PATHEXT"].split(";") : [""]
    end

    def find_output(source_path, output_extension)
      basenames = [File.basename(source_path, ".*"),
                   File.basename(source_path)]

      paths = basenames.map do |basename|
        "#{File.join(File.dirname(source_path), basename)}.#{output_extension}"
      end

      paths.find { |p| File.exist?(p) }
    end

    def raise_conversion_error(call)
      raise Vectory::ConversionError,
            "Could not convert with Inkscape. " \
            "Inkscape cmd: '#{call.cmd}',\n" \
            "status: '#{call.status}',\n" \
            "stdout: '#{call.stdout.strip}',\n" \
            "stderr: '#{call.stderr.strip}'."
    end

    def build_command(exe, input_path, output_path, output_format, plain)
      # Modern Inkscape (1.0+) uses --export-filename
      # Older versions use --export-<format>-file or --export-type
      if inkscape_version_modern?
        cmd = "#{exe} --export-filename=#{output_path}"
        cmd += " --export-plain-svg" if plain && output_format == :svg
        # For PDF input, specify which page to use (avoid interactive prompt)
        cmd += " --export-page=1" if input_path.end_with?(".pdf")
      else
        # Legacy Inkscape (0.x) uses --export-type
        cmd = "#{exe} --export-type=#{output_format}"
        cmd += " --export-plain-svg" if plain && output_format == :svg
      end
      cmd += " #{input_path}"
      cmd
    end

    def inkscape_version_modern?
      return @inkscape_version_modern if defined?(@inkscape_version_modern)

      exe = inkscape_path
      return @inkscape_version_modern = true unless exe # Default to modern

      version_output = `#{external_path(exe)} --version 2>&1`
      version_match = version_output.match(/Inkscape (\d+)\./)

      @inkscape_version_modern = if version_match
                                   version_match[1].to_i >= 1
                                 else
                                   true # Default to modern if we can't detect
                                 end
    end

    def with_temp_files(content, input_format, output_format)
      Dir.mktmpdir do |dir|
        input_path = File.join(dir, "image.#{input_format}")
        output_path = File.join(dir, "image.#{output_format}")
        File.binwrite(input_path, content)

        yield input_path, output_path
      end
    end

    def query_integer(content, format, options)
      query(content, format, options).to_f.round
    end

    def query(content, format, options)
      exe = inkscape_path_or_raise_error

      with_temp_file(content, format) do |path|
        cmd = "#{external_path(exe)} #{options} #{external_path(path)}"

        # Pass environment to disable display on non-Windows systems
        env = headless_environment
        call = SystemCall.new(cmd, env: env).call
        raise_query_error(call) if call.stdout.empty?

        call.stdout
      end
    end

    def with_temp_file(content, extension)
      Dir.mktmpdir do |dir|
        path = File.join(dir, "image.#{extension}")
        File.binwrite(path, content)

        yield path
      end
    end

    def raise_query_error(call)
      raise Vectory::InkscapeQueryError,
            "Could not query with Inkscape. " \
            "Inkscape cmd: '#{call.cmd}',\n" \
            "status: '#{call.status}',\n" \
            "stdout: '#{call.stdout.strip}',\n" \
            "stderr: '#{call.stderr.strip}'."
    end

    # Returns environment variables for headless operation
    # On non-Windows systems, disable DISPLAY to prevent X11/GDK initialization
    def headless_environment
      # On macOS/Linux, disable DISPLAY to prevent Gdk/X11 warnings
      Platform.windows? ? {} : { "DISPLAY" => "" }
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
