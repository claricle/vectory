# frozen_string_literal: true

source "https://rubygems.org"

# Specify your gem's dependencies in vectory.gemspec
gemspec

# Use ukiryu from feature/architecture-refactoring branch for Windows PowerShell fix
# Pin to specific commit for CI consistency
gem "ukiryu", github: "ukiryu/ukiryu", ref: "b5e4653e52356f0ab253e2c791d3b431bc16886a"

# Pin connection_pool to version compatible with Ruby 3.1
gem "connection_pool", "< 3.0"

# Pin minitest to version compatible with Ruby 3.1
gem "minitest", "< 6.0"

# Pin activesupport to version compatible with Ruby 3.1
gem "activesupport", "< 8.0"

# Pin public_suffix to version compatible with Ruby 3.1
gem "public_suffix", "< 6.0"

gem "canon", "~> 0.1.7"
gem "openssl", "~> 3.0"
gem "rake"
gem "rspec"
gem "rubocop"
gem "rubocop-performance"
gem "rubocop-rake"
gem "rubocop-rspec"
