# frozen_string_literal: true

# Shared helpers for the dual-engine specs (lane 3): the canonical step
# spec built from the transform manifest, and real ENV hygiene — no test
# doubles anywhere; the switch is driven through real environment values
# and the real (or absent) native library.
module RuntimeEnvHelpers
  RUNTIME_ENV_KEYS = %w[VECTORY_RUNTIME VECTORY_NATIVE_LIB].freeze

  # Saves the switch env, forgets the memoized probes, runs, restores.
  def with_fresh_runtime
    saved = RUNTIME_ENV_KEYS.to_h { |key| [key, ENV.fetch(key, nil)] }
    RUNTIME_ENV_KEYS.each { |key| ENV.delete(key) }
    Vectory::Runtime.reset!
    yield
  ensure
    saved.each do |key, value|
      ENV[key] = value
    end
    Vectory::Runtime.reset!
  end

  # Points the loader at a deterministic path that cannot exist: the
  # library is missing. (No temp-dir churn; the path is never created.)
  def with_missing_native_library
    ENV["VECTORY_NATIVE_LIB"] = File.join("", "nonexistent-vectory-spec", "libvectory_ffi.so")
    Vectory::Runtime.reset!
  end

  def with_native_library(path)
    ENV["VECTORY_NATIVE_LIB"] = path
    Vectory::Runtime.reset!
  end

  def with_runtime_mode(value)
    ENV["VECTORY_RUNTIME"] = value
  end
end

module CanonicalStepsHelpers
  # The transform manifest contract -> the canonical step spec (the shape
  # both engines interpret; lib/vectory/runtime_steps.rb).
  def canonical_steps(contract)
    contract.fetch("steps").map do |name|
      options = contract.fetch("options", {})
      case name
      when "graft_into"
        { "name" => "graft_into", "host_content" => transform_fixture(contract.fetch("host")) }
      when "disambiguate"
        { "name" => "disambiguate", "index" => options.fetch("index"),
          "duplicate_ids" => options.fetch("duplicate_ids") }
      when "suffix_classes"
        { "name" => "suffix_classes", "index" => options.fetch("index") }
      when "bind_namespaces"
        { "name" => "bind_namespaces", "namespaces" => options.fetch("namespaces") }
      else
        raise ArgumentError, "unknown manifest step #{name.inspect}"
      end
    end
  end
end

RSpec.configure do |config|
  config.include RuntimeEnvHelpers
  config.include CanonicalStepsHelpers
end
