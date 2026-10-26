# frozen_string_literal: true

require "stringio"

module Vectory
  module PngCliHelpers
    def capture_png_cli(arguments)
      previous_stderr = $stderr
      $stderr = StringIO.new
      status = described_class.start(arguments)
      [status, $stderr.string]
    ensure
      $stderr = previous_stderr
    end

    def png_cli_arguments(input, output, *options)
      ["-f", "png", "-o", output, *options, input]
    end

    def png_cli_warning(family, scalar)
      Svgpng::FontSubstitutionWarning.new(
        requested_family: family, selected_family: "Fallback", style: "normal",
        weight: 400, stretch: "normal", scalars: [scalar]
      )
    end
  end
end

RSpec.configure do |config|
  config.include Vectory::PngCliHelpers, :png_cli
end
