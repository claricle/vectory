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
  puts "[VECTORY DEBUG] Checking register at: #{cached_register}"
  puts "[VECTORY DEBUG] Register exists: #{Dir.exist?(cached_register)}"

  if Dir.exist?(cached_register)
    puts "[VECTORY DEBUG] Deleting cached register..."
    FileUtils.rm_rf(cached_register)
    puts "[VECTORY DEBUG] Register deleted. Exists now: #{Dir.exist?(cached_register)}"
  end

  # Enable Vectory debug output on Windows CI to diagnose issues
  # Use RUBY_PLATFORM check since Gem.win_platform? might not be available yet
  if RUBY_PLATFORM =~ /mswin|mingw|cygwin/
    ENV["VECTORY_DEBUG"] = "true"
  end
end

require "vectory"
require "rspec/matchers"
require "canon"

# Debug: Log register information and batch_process default value
if ENV["CI"] && RUBY_PLATFORM =~ /mswin|mingw|cygwin/
  begin
    # Check register path and source
    register = Ukiryu::Register.default
    puts "[VECTORY DEBUG] Register path: #{register.path}"
    puts "[VECTORY DEBUG] Register source: #{register.source}"

    # Check if the register has the fix by reading the YAML file directly
    inkscape_yaml = File.join(register.path, "tools", "inkscape", "1.0.yaml")
    if File.exist?(inkscape_yaml)
      content = File.read(inkscape_yaml)
      puts "[VECTORY DEBUG] YAML file found at: #{inkscape_yaml}"

      # Parse YAML to verify the value
      require 'yaml'
      hash = YAML.safe_load(content, permitted_classes: [Symbol], aliases: true)
      if hash && hash['profiles']
        modern_unix = hash['profiles'].find { |p| p['name'] == 'modern_unix' }
        if modern_unix && modern_unix['commands']
          export_cmd = modern_unix['commands'].find { |c| c['name'] == 'export' }
          if export_cmd && export_cmd['flags']
            batch_flag = export_cmd['flags'].find { |f| f['name'] == 'batch_process' }
            if batch_flag
              puts "[VECTORY DEBUG] YAML modern_unix batch_process default: #{batch_flag['default'].inspect} (#{batch_flag['default'].class})"
            end
          end
        end
      end
    else
      puts "[VECTORY DEBUG] Inkscape YAML not found at: #{inkscape_yaml}"
    end

    # Check loaded tool definition
    puts "[VECTORY DEBUG] Loading Inkscape tool..."
    inkscape_tool = Ukiryu::Tool.get(:inkscape)

    # Access the internal profile structure
    profile = inkscape_tool.instance_variable_get(:@profile)
    puts "[VECTORY DEBUG] Profile class: #{profile.class}"
    puts "[VECTORY DEBUG] Profile name: #{profile.name}"
    puts "[VECTORY DEBUG] Profile commands count: #{profile.commands&.length || 'nil'}"

    if profile.commands
      export_cmd = profile.commands.find { |c| c.name == "export" }
      puts "[VECTORY DEBUG] Export command found: #{!export_cmd.nil?}"
      if export_cmd && export_cmd.flags
        batch_flag = export_cmd.flags.find { |f| f.name == "batch_process" || f.name == :batch_process }
        if batch_flag
          puts "[VECTORY DEBUG] Loaded batch_process default: #{batch_flag.default.inspect} (#{batch_flag.default.class})"
          puts "[VECTORY DEBUG] Loaded batch_process cli: #{batch_flag.cli.inspect}"
        else
          puts "[VECTORY DEBUG] batch_process flag NOT FOUND in loaded export command!"
          puts "[VECTORY DEBUG] Available flags: #{export_cmd.flags.map(&:name).inspect}"
        end
      else
        puts "[VECTORY DEBUG] Export command has no flags"
      end
    end
  rescue => e
    puts "[VECTORY DEBUG] Error: #{e.class} - #{e.message}"
    puts "[VECTORY DEBUG] Backtrace: #{e.backtrace.first(5).join("\n")}"
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
