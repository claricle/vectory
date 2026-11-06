# frozen_string_literal: true

require "spec_helper"
require "vectory/cli"

RSpec.describe Vectory::CLI, :png_input_matrix do
  let(:svg) do
    Vectory::Svg.from_content(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">' \
      '<rect width="20" height="10" fill="red"/></svg>',
    )
  end

  before { Vectory.ui.level = :fatal }

  %w[svg emf eps ps].each do |format|
    it "renders actual #{format.upcase} input through the packaged native renderer" do
      Dir.mktmpdir do |directory|
        input = File.join(directory, "source.#{format}")
        output = File.join(directory, "output.png")
        image = format == "svg" ? svg : svg.public_send("to_#{format}")
        File.binwrite(input, image.content)

        status = described_class.start(["convert", input, "--format", "png", "--output", output])

        expect(status).to eq(described_class::STATUS_SUCCESS)
        png = Vectory::Png.from_path(output)
        expect(png.content).to start_with("\x89PNG\r\n\x1a\n".b)
        expect([png.width, png.height]).to eq([20, 10])
        expect(png.warnings).to eq([])
      end
    end
  end
end
