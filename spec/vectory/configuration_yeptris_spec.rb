# frozen_string_literal: true

require "spec_helper"
require "open3"
require "rbconfig"

RSpec.describe Vectory::Configuration do
  it "loads configuration without requiring Psych" do
    script = <<~RUBY
      require "vectory"
      config = Vectory::Configuration.load_from_file(ARGV.fetch(0))
      abort "Configuration was not loaded" unless config.timeout == 60
      abort "Psych was loaded" if defined?(::Psych)
    RUBY
    fixture = File.expand_path("../fixtures/configuration/full.yml", __dir__)
    stdout, stderr, status = Open3.capture3(
      { "RUBYOPT" => nil },
      RbConfig.ruby, "-I", File.expand_path("../../lib", __dir__),
      "-e", script, fixture
    )

    expect(status.success?).to be(true), "#{stdout}\n#{stderr}"
  end
end
