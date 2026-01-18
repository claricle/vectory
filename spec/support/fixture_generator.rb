# frozen_string_literal: true

require "open3"
require "tempfile"

# FixtureGenerator regenerates test fixture reference files using external tools
# This ensures fixtures stay in sync with external tool versions
# (cairo, ghostscript, etc.)
#
# IMPORTANT: This must use external tools directly (Ghostscript, Inkscape),
# NOT Vectory. The fixtures are "golden masters" that Vectory is tested against.
module FixtureGenerator
  class << self
    def generate_all
      generate_ps_fixtures
    end

    def generate_ps_fixtures
      puts "  Generating PS fixtures using external tools..."

      # PS to EPS via Ghostscript + Inkscape (not via Vectory)
      generate_ps_to_eps_fixture

      # PS to SVG via Ghostscript + Inkscape (not via Vectory)
      generate_ps_to_svg_fixture

      puts "  PS fixtures done."
    end

    private

    # Generate EPS reference by calling Ghostscript + Inkscape directly
    # (PS → PDF via Ghostscript, then PDF → EPS via Inkscape)
    def generate_ps_to_eps_fixture
      input_file = "spec/examples/ps2eps/img.ps"
      output_file = "spec/examples/ps2eps/ref.eps"

      unless File.exist?(input_file)
        puts "    SKIP: #{input_file} not found"
        return
      end

      # Step 1: Convert PS to PDF using Ghostscript
      Tempfile.create(["gs_pdf", ".pdf"]) do |pdf_temp|
        ps_to_pdf_via_ghostscript(input_file, pdf_temp.path)

        # Step 2: Convert PDF to EPS using Inkscape directly
        eps_content = pdf_to_eps_via_inkscape(pdf_temp.path)

        # Write reference file
        File.write(output_file, eps_content)
        puts "    Generated #{output_file} " \
             "(PS→PDF→EPS via Ghostscript+Inkscape)"
      end
    end

    # Generate SVG reference by calling Ghostscript + Inkscape directly
    def generate_ps_to_svg_fixture
      input_file = "spec/examples/ps2svg/img.ps"
      output_file = "spec/examples/ps2svg/ref.svg"

      unless File.exist?(input_file)
        puts "    SKIP: #{input_file} not found"
        return
      end

      # Step 1: Convert PS to PDF using Ghostscript
      Tempfile.create(["gs_pdf", ".pdf"]) do |pdf_temp|
        ps_to_pdf_via_ghostscript(input_file, pdf_temp.path)

        # Step 2: Convert PDF to SVG using Inkscape directly
        svg_content = pdf_to_svg_via_inkscape(pdf_temp.path)

        # Write reference file
        File.write(output_file, svg_content)
        puts "    Generated #{output_file} " \
             "(PS→PDF→SVG via Ghostscript+Inkscape)"
      end
    end

    def ps_to_pdf_via_ghostscript(input_file, output_path)
      ps_content = File.read(input_file)

      Tempfile.create(["gs_input", ".ps"]) do |input_temp|
        input_temp.binmode
        input_temp.write(ps_content)
        input_temp.flush
        input_temp.close

        # Build Ghostscript command
        gs_exe = find_ghostscript_executable
        cmd = build_ghostscript_command(gs_exe, input_temp.path, output_path,
                                        eps_crop: true)

        execute_command(cmd)

        pdf_content = File.binread(output_path)
        raise "Ghostscript failed: empty PDF" if pdf_content.size < 100

        pdf_content
      end
    end

    def pdf_to_svg_via_inkscape(pdf_path)
      # Build Inkscape command directly
      inkscape_exe = find_inkscape_executable
      Tempfile.create(["inkscape_output", ".svg"]) do |svg_temp|
        svg_temp.close

        cmd = [
          inkscape_exe,
          "--export-filename=#{svg_temp.path}",
          "--export-plain-svg",
          pdf_path,
        ]

        # Set headless environment
        env = headless_environment
        if windows?
          Open3.capture3(*cmd, env)
        else
          Open3.capture3(env, *cmd)
        end

        # result[0] = stdout, result[1] = stderr, result[2] = status
        # Inkscape outputs warnings to stderr but they're not failures
        # Only fail if the SVG wasn't created
        svg_content = File.read(svg_temp.path)
        raise "Inkscape failed: empty SVG" if svg_content.size < 100

        svg_content
      end
    end

    def pdf_to_eps_via_inkscape(pdf_path)
      # Build Inkscape command directly
      inkscape_exe = find_inkscape_executable
      Tempfile.create(["inkscape_output", ".eps"]) do |eps_temp|
        eps_temp.close

        cmd = [
          inkscape_exe,
          "--export-filename=#{eps_temp.path}",
          "--export-type=eps",
          pdf_path,
        ]

        # Set headless environment
        env = headless_environment
        if windows?
          Open3.capture3(*cmd, env)
        else
          Open3.capture3(env, *cmd)
        end

        # Inkscape outputs warnings to stderr but they're not failures
        # Only fail if the EPS wasn't created
        eps_content = File.read(eps_temp.path)
        raise "Inkscape failed: empty EPS" if eps_content.size < 100

        eps_content
      end
    end

    def find_ghostscript_executable
      if windows?
        # Try common Windows paths
        paths = [
          "C:/Program Files/gs/gs*/bin/gswin64c.exe",
          "C:/Program Files (x86)/gs/gs*/bin/gswin32c.exe",
        ]
        paths.each do |pattern|
          found = Dir.glob(pattern).max
          return found if found
        end
        "gswin64c.exe"
      else
        "gs"
      end
    end

    def find_inkscape_executable
      if windows?
        "inkscape.exe"
      else
        "inkscape"
      end
    end

    def build_ghostscript_command(exe, input_path, output_path, options = {})
      cmd_parts = []
      cmd_parts << exe

      # Use different devices based on output file extension
      cmd_parts << if output_path.end_with?(".eps")
                     # For EPS output, use epswrite device
                     "-sDEVICE=epswrite"
                   else
                     # For PDF output, use pdfwrite device
                     "-sDEVICE=pdfwrite"
                   end

      cmd_parts << "-dNOPAUSE"
      cmd_parts << "-dBATCH"
      cmd_parts << "-dSAFER"
      cmd_parts << "-sOutputFile=#{output_path}"
      cmd_parts << "-dEPSCrop" if options[:eps_crop]
      cmd_parts << "-dAutoRotatePages=/None"
      cmd_parts << "-dQUIET"
      cmd_parts << "-f"
      cmd_parts << input_path
      cmd_parts
    end

    def execute_command(cmd, check_stderr: true)
      result = if cmd.is_a?(Array)
                 Open3.capture3(*cmd)
               else
                 Open3.capture3(cmd)
               end

      # result[0] = stdout, result[1] = stderr, result[2] = status
      # Some tools output warnings to stderr but still succeed
      if check_stderr && !result[1].empty?
        raise "Command failed: #{cmd}\nstderr: #{result[1]}"
      end

      result
    end

    def windows?
      !!((RUBY_PLATFORM =~ /(win|w)(32|64)$/) ||
         (RUBY_PLATFORM =~ /mswin|mingw/))
    end

    def headless_environment
      windows? ? {} : { "DISPLAY" => "" }
    end

    def strip_image(xml)
      # Handle both self-closing and regular image tags with content
      xml.gsub(%r{<image\b[^>]*>.*?</image>}m, "<image/>")
        .gsub(%r{<image\b[^>]*/>}, "<image/>")
    end
  end
end
