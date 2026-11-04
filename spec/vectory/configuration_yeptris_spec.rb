# frozen_string_literal: true

require "spec_helper"
require "open3"
require "rbconfig"

RSpec.describe Vectory::Configuration do
  it "loads configuration within the bundle without loading additional Psych files" do
    script = <<~RUBY
      abort "Bundler was not set up" unless defined?(Bundler)
      psych_features = lambda do
        $LOADED_FEATURES.grep(%r{/psych[/.]}).reject do |feature|
          feature.include?("/yeptris/")
        end
      end
      baseline = psych_features.call
      require "vectory"
      abort "Requiring Vectory loaded Psych" unless psych_features.call == baseline
      config = Vectory::Configuration.load_from_file(ARGV.fetch(0))
      abort "Configuration was not loaded" unless config.timeout == 60
      abort "Loading configuration loaded Psych" unless psych_features.call == baseline
    RUBY
    fixture = File.expand_path("../fixtures/configuration/full.yml", __dir__)
    stdout, stderr, status = Open3.capture3(
      { "RUBYOPT" => nil },
      RbConfig.ruby, "-rbundler/setup",
      "-I", File.expand_path("../../lib", __dir__),
      "-e", script, fixture
    )

    expect(status.success?).to be(true), "#{stdout}\n#{stderr}"
  end
end
