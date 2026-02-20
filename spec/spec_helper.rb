# frozen_string_literal: true

require "tmpdir"
require "fileutils"

# Configure Ukiryu register path
# On CI, force a fresh clone of the register to get the latest tool definitions
# with the batch_process fix (default: false for headless CI)
# This must be done BEFORE Ukiryu is loaded
if ENV["CI"]
  # Delete any cached register to force fresh clone from GitHub
  cached_register = File.expand_path("~/.ukiryu/register")
  FileUtils.rm_rf(cached_register) if Dir.exist?(cached_register)

  # Enable Vectory debug output on Windows CI to diagnose issues
  # Use RUBY_PLATFORM check since Gem.win_platform? might not be available yet
  if RUBY_PLATFORM =~ /mswin|mingw|cygwin/
    ENV["VECTORY_DEBUG"] = "true"
  end
end

require "vectory"
require "rspec/matchers"
require "canon"

# Debug: Log the batch_process default value to verify the register fix is applied
if ENV["CI"] && RUBY_PLATFORM =~ /mswin|mingw|cygwin/
  begin
    inkscape_tool = Ukiryu::Tool.get(:inkscape)
    export_cmd = inkscape_tool.command_profile.command("export")
    batch_flag = export_cmd.flags&.find { |f| f.name_sym == :batch_process }
    if batch_flag
      puts "[VECTORY DEBUG] Inkscape batch_process default: #{batch_flag.default.inspect}"
    else
      puts "[VECTORY DEBUG] WARNING: batch_process flag not found in inkscape export command"
    end
  rescue => e
    puts "[VECTORY DEBUG] Error checking batch_process default: #{e.message}"
  end
end

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
