# frozen_string_literal: true

require "tmpdir"
require "fileutils"

# Configure Ukiryu register path
# On CI, force a fresh clone of the register to get the latest tool definitions
# with the Windows profile fix (inherits: unix)
# This must be done BEFORE Ukiryu is loaded
if ENV["CI"]
  # Delete any cached register to force fresh clone
  cached_register = File.expand_path("~/.ukiryu/register")
  if Dir.exist?(cached_register)
    puts "[VECTORY DEBUG] Deleting cached register at: #{cached_register}"
    FileUtils.rm_rf(cached_register)
    puts "[VECTORY DEBUG] Cached register deleted"
  end

  # Set UKIRYU_DEBUG to see register cloning
  ENV["UKIRYU_DEBUG"] = "1"
end

require "vectory"

# Debug: Show register info after loading
if ENV["CI"]
  begin
    register = Ukiryu::Register.default
    puts "[VECTORY DEBUG] Register path: #{register.path}"
    puts "[VECTORY DEBUG] Register source: #{register.source}"

    # Show git info
    git_dir = File.join(register.path, ".git")
    if Dir.exist?(git_dir)
      git_log = `cd "#{register.path}" && git log -1 --oneline 2>&1`.strip
      puts "[VECTORY DEBUG] Register git commit: #{git_log}"
    end

    # Show ghostscript default/10.0.yaml Windows profile content
    gs_file = File.join(register.path, "tools/ghostscript/default/10.0.yaml")
    if File.exist?(gs_file)
      content = File.read(gs_file)
      # Extract Windows profile section
      if content =~ /(- name: windows.*?)(?=\n- name:|\nsmoke_tests:|\n\z)/m
        windows_section = Regexp.last_match(1)
        puts "[VECTORY DEBUG] Ghostscript Windows profile from file:"
        puts windows_section.lines.first(15).join
      end
    else
      puts "[VECTORY DEBUG] File not found: #{gs_file}"
    end

    # Show loaded implementation version profiles
    begin
      impl_version = register.load_implementation_version("ghostscript", "default", "10.0.yaml")
      if impl_version && impl_version.execution_profiles
        puts "[VECTORY DEBUG] Loaded execution_profiles count: #{impl_version.execution_profiles.count}"
        impl_version.execution_profiles.each do |profile|
          profile_name = profile[:name] || profile["name"]
          profile_inherits = profile[:inherits] || profile["inherits"]
          profile_keys = profile.keys.inspect
          puts "[VECTORY DEBUG] Profile '#{profile_name}' - inherits: #{profile_inherits.inspect}, keys: #{profile_keys}"
        end
      end
    rescue => e
      puts "[VECTORY DEBUG] Error loading implementation version: #{e.message}"
    end
  rescue => e
    puts "[VECTORY DEBUG] Error getting register info: #{e.message}"
  end
end

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
