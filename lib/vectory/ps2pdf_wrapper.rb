# frozen_string_literal: true

require "tempfile"
require "fileutils"
require_relative "errors"
require_relative "system_call"

module Vectory
  class Ps2pdfWrapper
    SUPPORTED_INPUT_FORMATS = %w[ps eps].freeze

    class << self
      def available?
        ps2pdf_path
        true
      rescue Ps2pdfNotFoundError
        false
      end

      def version
        return nil unless available?

        cmd = "#{ps2pdf_path} --version"
        call = SystemCall.new(cmd).call
        call.stdout.strip
      rescue StandardError
        nil
      end

      def convert(content, options = {})
        raise Ps2pdfNotFoundError unless available?

        eps_crop = options.fetch(:eps_crop, false)
        input_ext = eps_crop ? ".eps" : ".ps"

        # Create temporary input file with the content
        Tempfile.create(["ps2pdf_input", input_ext]) do |input_file|
          input_file.write(content)
          input_file.flush
          input_file.close

          # Create temporary output file
          Tempfile.create(["ps2pdf_output", ".pdf"]) do |output_file|
            output_file.close

            cmd = build_command(input_file.path, output_file.path,
                                eps_crop: eps_crop)

            begin
              SystemCall.new(cmd).call
            rescue SystemCallError => e
              raise ConversionError,
                    "ps2pdf conversion failed: #{e.message}"
            end

            unless File.exist?(output_file.path)
              raise ConversionError,
                    "ps2pdf did not create output file: #{output_file.path}"
            end

            File.read(output_file.path)
          end
        end
      end

      private

      def ps2pdf_path
        path = which("ps2pdf")
        raise Ps2pdfNotFoundError unless path

        path
      end

      def which(cmd)
        exts = ENV["PATHEXT"] ? ENV["PATHEXT"].split(";") : [""]
        ENV["PATH"].split(File::PATH_SEPARATOR).each do |path|
          exts.each do |ext|
            exe = File.join(path, "#{cmd}#{ext}")
            return exe if File.executable?(exe) && !File.directory?(exe)
          end
        end
        nil
      end

      def build_command(input_path, output_path, options = {})
        cmd_parts = [ps2pdf_path]

        # Add EPS crop option if specified (for EPS files)
        cmd_parts << "-dEPSCrop" if options[:eps_crop]

        # Always set autorotate to None to avoid interactive prompts
        cmd_parts << "-dAutoRotatePages=/None"

        # Add input and output paths
        cmd_parts << input_path
        cmd_parts << output_path

        cmd_parts.join(" ")
      end
    end
  end
end
