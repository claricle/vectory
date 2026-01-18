# frozen_string_literal: true

require "bundler/gem_tasks"
require "rspec/core/rake_task"

RSpec::Core::RakeTask.new(:spec)

task default: :spec

# require "rubocop/rake_task"

# RuboCop::RakeTask.new

# task default: %i[spec rubocop]

desc "Regenerate test fixture reference files"
task :regenerate_fixtures do
  require "fileutils"
  require_relative "spec/support/fixture_generator"

  puts "Regenerating test fixtures..."
  FixtureGenerator.generate_all
  puts "Fixtures regenerated successfully!"
rescue LoadError => e
  abort "Error: #{e.message}\n" \
       "Make sure all dependencies are installed: bundle install"
end
