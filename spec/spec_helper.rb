# frozen_string_literal: true

# Configure Ukiryu registry path for local development
# This is needed because we use a local path dependency for ukiryu in the Gemfile
ENV["UKIRYU_REGISTRY"] ||= File.expand_path("../../../ukiryu/register", __dir__)

require "vectory"
require "tmpdir"
require "rspec/matchers"
require "canon"

Dir["./spec/support/**/*.rb"].sort.each { |file| require file }

# Configure Canon RSpec matchers to use whitespace normalization
Canon::RSpecMatchers.configure do |config|
  config.xml_match_profile = :spec_friendly
end

RSpec.configure do |config|
  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status"

  config.include Vectory::Helper

  # Disable RSpec exposing methods globally on `Module` and `main`
  config.disable_monkey_patching!

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end
end
