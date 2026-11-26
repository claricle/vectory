# frozen_string_literal: true

require "bundler/gem_tasks"
require "rspec/core/rake_task"

RSpec::Core::RakeTask.new(:spec)

task default: :spec

# rb-sys-dock's default cross-gem build command is
# `rake native:$RUBY_TARGET gem` (oxidize-rb/rb-sys gem/exe/rb-sys-dock):
# the native:<platform> tasks (rakelib/native.rake) package the platform
# gem into pkg/, and this — its tail — proves the source gem still builds
# binary-free beside it (the gemspec guard holds without
# VECTORY_VENDOR_CDYLIB).
desc "Build the binary-free source gem (the rb-sys-dock build command's tail)"
task :gem do
  sh "gem", "build", "vectory.gemspec"
end

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
