require "thor"
require "vectory"

module Vectory
  class CLI < Thor
    STATUS_SUCCESS = 0
    STATUS_UNKNOWN_ERROR = 1
    STATUS_UNSUPPORTED_INPUT_FORMAT_ERROR = 2
    STATUS_UNSUPPORTED_OUTPUT_FORMAT_ERROR = 3
    STATUS_CONVERSION_ERROR = 4
    STATUS_SYSTEM_CALL_ERROR = 5
    STATUS_INKSCAPE_NOT_FOUND_ERROR = 6
    STATUS_SAME_FORMAT_ERROR = 7
    STATUS_GHOSTSCRIPT_NOT_FOUND_ERROR = 8

    MAP_ERROR_TO_STATUS = {
      Vectory::ConversionError => STATUS_CONVERSION_ERROR,
      Vectory::InkscapeNotFoundError => STATUS_INKSCAPE_NOT_FOUND_ERROR,
      Vectory::GhostscriptNotFoundError => STATUS_GHOSTSCRIPT_NOT_FOUND_ERROR,
      Vectory::SystemCallError => STATUS_SYSTEM_CALL_ERROR,
    }.freeze

    module SupportedInputFormats
      EPS = :eps
      PS  = :ps
      SVG = :svg
      EMF = :emf

      def self.all
        constants.map { |x| const_get(x) }
      end
    end

    module SupportedOutputFormats
      EPS = "eps".freeze
      PS  = "ps".freeze
      SVG = "svg".freeze
      EMF = "emf".freeze

      def self.all
        constants.map { |x| const_get(x) }
      end
    end

    def self.exit_on_failure?
      false
    end

    desc "convert INPUT_FILE_NAME",
         "Perform pairwise vector image conversions for common vector image " \
         "formats (EPS, PS, EMF, SVG)"
    option :format,
           aliases: :f,
           required: true,
           desc: "the desired output format (one of: svg, eps, ps, emf)"

    option :output,
           aliases: :o,
           desc: "file path to the desired output file (with the file " \
                 "extension)"
    def convert(file)
      unless supported_format?(options[:format])
        return unsupported_format_error(options[:format])
      end

      input_format = detect_input_format(file)
      if same_format?(input_format, options[:format])
        return same_format_error(options[:format])
      end

      unless supported_input_format?(input_format)
        return unsupported_input_format_error
      end

      object = source_object(file, input_format)

      convert_to_format(object, options)
    end
    default_task :convert

    private

    def supported_format?(format)
      SupportedOutputFormats.all.include?(format)
    end

    def unsupported_format_error(format)
      formats = SupportedOutputFormats.all.map { |v| "'#{v}'" }.join(", ")

      Vectory.ui.error(
        "Unsupported output format '#{format}'. " \
        "Please choose one of: #{formats}.",
      )

      STATUS_UNSUPPORTED_OUTPUT_FORMAT_ERROR
    end

    def detect_input_format(file)
      FileMagic.detect(file)
    end

    def same_format?(input_format, output_format)
      input_format.to_s == output_format
    end

    def same_format_error(format)
      Vectory.ui.error("Could not convert to the same format '#{format}'.")

      STATUS_SAME_FORMAT_ERROR
    end

    def supported_input_format?(format)
      SupportedInputFormats.all.include?(format)
    end

    def unsupported_input_format_error
      formats = SupportedInputFormats.all.map { |v| "'#{v}'" }.join(", ")
      Vectory.ui.error(
        "Could not detect input format. " \
        "Please provide file of the following formats: #{formats}.",
      )

      STATUS_UNSUPPORTED_INPUT_FORMAT_ERROR
    end

    def source_object(file, format)
      Vectory.const_get(format.capitalize).from_path(file)
    end

    def convert_to_format(object, options)
      path = options[:output] || current_dir_path(object.initial_path,
                                                  options[:format])
      to_format(object, options[:format]).write(path)
      Vectory.ui.info("Output file was written to #{path}")

      STATUS_SUCCESS
    rescue Vectory::Error => e
      handle_vectory_error(e)
    end

    def handle_vectory_error(exception)
      Vectory.ui.error(exception.message)

      MAP_ERROR_TO_STATUS[exception.class] || raise(exception)
    end

    def current_dir_path(input_path, output_format)
      File.join(Dir.pwd, "#{File.basename(input_path, '.*')}.#{output_format}")
    end

    def to_format(object, format)
      method_name = "to_#{format}"
      object.public_send(method_name)
    end
  end
end
