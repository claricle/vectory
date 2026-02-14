# frozen_string_literal: true

require "tempfile"
require "fileutils"
require "ukiryu"

module Vectory
  # GhostscriptWrapper converts PS and EPS files to PDF using Ghostscript
  #
  # Uses Ukiryu for platform-adaptive command execution.
  class GhostscriptWrapper
    SUPPORTED_INPUT_FORMATS = %w[ps eps].freeze

    # Configure Ukiryu registry path
    @registry_path = nil

    class << self
      attr_accessor :registry_path

      def available?
        configure_registry
        ghostscript_tool
        true
      rescue GhostscriptNotFoundError
        false
      end

      def version
        return nil unless available?

        tool = ghostscript_tool
        tool.version
      rescue StandardError
        nil
      end

      def convert(content, options = {})
        configure_registry
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

          # Get the tool and execute
          tool = ghostscript_tool
          params = build_convert_params(input_file.path, output_file.path,
                                        eps_crop: eps_crop)

          result = tool.execute(:convert,
                                execution_timeout: Configuration.instance.timeout,
                                **params)

          unless result.success?
            raise ConversionError,
                  "GhostScript conversion failed. " \
                  "Command: #{result.command}, " \
                  "Exit status: #{result.status}, " \
                  "stdout: '#{result.stdout.strip}', " \
                  "stderr: '#{result.stderr.strip}'"
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
                  "Command: #{result.command}, " \
                  "stdout: '#{result.stdout.strip}', " \
                  "stderr: '#{result.stderr.strip}'"
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

      # Configure the Ukiryu registry path
      def configure_registry
        return if @registry_configured

        # Explicit path takes precedence
        if @registry_path
          Ukiryu::Register.default_register_path = @registry_path
        elsif ENV["UKIRYU_REGISTER"]
          Ukiryu::Register.default_register_path = ENV["UKIRYU_REGISTER"]
        else
          # Ensure register is available (auto-clones if needed)
          ensure_register_available
        end

        @registry_configured = true
      end

      # Ensure the ukiryu register is available
      # This triggers the auto-clone mechanism if needed
      def ensure_register_available
        # First check if UKIRYU_REGISTER is set and directory exists
        env_path = ENV["UKIRYU_REGISTER"]
        if env_path && Dir.exist?(env_path)
          Ukiryu::Register.default_register_path = env_path
          return
        end

        # Try to use RegisterAutoManager's auto-clone mechanism
        begin
          register_path = Ukiryu::RegisterAutoManager.register_path
          if register_path && Dir.exist?(register_path)
            Ukiryu::Register.default_register_path = register_path
            return
          end
        rescue StandardError => e
          # Auto-clone failed, try manual clone
          warn "[Vectory] RegisterAutoManager failed: #{e.message}"
        end

        # If we get here, try manual clone to a default location
        manual_clone_register
      end

      # Manually clone the register if all else fails
      def manual_clone_register
        require "fileutils"

        # Use a consistent location for the register
        register_path = File.expand_path("~/.ukiryu/register")

        return if Dir.exist?(register_path)

        parent_dir = File.dirname(register_path)
        FileUtils.mkdir_p(parent_dir) unless Dir.exist?(parent_dir)

        register_url = "https://github.com/ukiryu/register"

        # Try to clone using system git
        success = system("git clone --depth 1 #{register_url} #{register_path}")
        success ||= system('"C:\Program Files\Git\bin\git.exe" clone --depth 1 ' \
                           "#{register_url} #{register_path}")

        if success && Dir.exist?(register_path)
          Ukiryu::Register.default_register_path = register_path
        else
          warn "[Vectory] Warning: Failed to clone ukiryu register"
        end
      rescue StandardError => e
        warn "[Vectory] Warning: Failed to setup ukiryu register: #{e.message}"
      end

      # Get the Ghostscript tool from Ukiryu
      def ghostscript_tool
        Ukiryu::Tool.get("ghostscript")
      rescue Ukiryu::Errors::ToolNotFoundError => e
        # Tool not found - raise the original GhostscriptNotFoundError
        raise GhostscriptNotFoundError, "Ghostscript not available: #{e.message}"
      end

      # Build convert parameters for Ukiryu
      def build_convert_params(input_path, output_path, options = {})
        params = {
          inputs: [input_path],
          device: :pdfwrite,
          output: output_path,
          batch: true,
          no_pause: true,
          quiet: true,
        }

        # Add EPS crop option
        params[:eps_crop] = true if options[:eps_crop]

        params
      end
    end
  end
end
