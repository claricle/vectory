# frozen_string_literal: true

require "tempfile"
require "fileutils"
require_relative "errors"
require_relative "system_call"
require_relative "platform"
require_relative "system_command"

module Vectory
  # GhostscriptWrapper converts PS and EPS files to PDF using Ghostscript
  class GhostscriptWrapper
    SUPPORTED_INPUT_FORMATS = %w[ps eps].freeze

    class << self
      def available?
        ghostscript_path
        true
      rescue GhostscriptNotFoundError
        false
      end

      def version
        return nil unless available?

        cmd = [ghostscript_path, "--version"]
        call = SystemCall.new(cmd).call
        call.stdout.strip
      rescue StandardError
        nil
      end

      def convert(content, options = {})
        raise GhostscriptNotFoundError unless available?

        eps_crop = options.fetch(:eps_crop, false)
        input_ext = eps_crop ? ".eps" : ".ps"

        # Create temporary input file
        input_file = Tempfile.new(["gs_input", input_ext])
        output_file = Tempfile.new(["gs_output", ".pdf"])

        begin
          # Write content and close the input file so GhostScript can read it
          input_file.binmode
          input_file.write(content)
          input_file.flush
          input_file.close

          # Close output file so GhostScript can write to it
          output_file.close

          cmd = build_command(input_file.path, output_file.path,
                              eps_crop: eps_crop)

          call = nil
          begin
            call = SystemCall.new(cmd).call
          rescue SystemCallError => e
            raise ConversionError,
                  "GhostScript conversion failed: #{e.message}"
          end

          unless File.exist?(output_file.path)
            raise ConversionError,
                  "GhostScript did not create output file: #{output_file.path}"
          end

          output_content = File.binread(output_file.path)

          # Check if the PDF is valid (should be more than just the header)
          if output_content.size < 100
            raise ConversionError,
                  "GhostScript created invalid PDF (#{output_content.size} bytes). " \
                  "Command: #{cmd.join(' ')}, " \
                  "stdout: '#{call&.stdout&.strip}', " \
                  "stderr: '#{call&.stderr&.strip}'"
          end

          output_content
        ensure
          # Clean up temp files
          input_file.close unless input_file.closed?
          input_file.unlink
          output_file.close unless output_file.closed?
          output_file.unlink
        end
      end

      private

      def ghostscript_path
        # First try common installation paths specific to each platform
        if Platform.windows?
          # Check common Windows installation directories first
          common_windows_paths = [
            "C:/Program Files/gs/gs*/bin/gswin64c.exe",
            "C:/Program Files (x86)/gs/gs*/bin/gswin32c.exe",
          ]

          common_windows_paths.each do |pattern|
            Dir.glob(pattern).sort.reverse.each do |path|
              return path if File.executable?(path)
            end
          end

          # Then try PATH for Windows executables
          ["gswin64c.exe", "gswin32c.exe", "gs"].each do |cmd|
            path = SystemCommand.find_executable(cmd)
            return path if path
          end
        else
          # On Unix-like systems, check PATH
          path = SystemCommand.find_executable("gs")
          return path if path
        end

        raise GhostscriptNotFoundError
      end

      def build_command(input_path, output_path, options = {})
        cmd_parts = []
        cmd_parts << ghostscript_path
        cmd_parts << "-sDEVICE=pdfwrite"
        cmd_parts << "-dNOPAUSE"
        cmd_parts << "-dBATCH"
        cmd_parts << "-dSAFER"
        # Use separate arguments for output file to ensure proper path handling
        cmd_parts << "-sOutputFile=#{output_path}"
        cmd_parts << "-dEPSCrop" if options[:eps_crop]
        cmd_parts << "-dAutoRotatePages=/None"
        cmd_parts << "-dQUIET"
        # Use -f to explicitly specify input file
        cmd_parts << "-f"
        cmd_parts << input_path

        cmd_parts
      end
    end
  end
end
