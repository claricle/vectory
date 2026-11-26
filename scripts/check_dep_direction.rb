#!/usr/bin/env ruby
# frozen_string_literal: true

# Dependency-direction guard for the vectory-rs workspace.
#
# The owner's model-driven law: format models must not depend on transformers,
# engines, bindings, or each other. The `vectory` crate (the SVG format model)
# must never depend on the VIM (`eikogram`) - the VIM->SVG transformer is
# the only bridge between them. The VIM must have zero format-model
# dependencies. A transformer may depend only on the models it bridges (plus
# the VIM for VIM-emitting transformers). Models are leaves of the workspace
# graph. The graph is read mechanically from `cargo metadata`; every workspace
# crate must be classified in RULES below, so adding a crate forces an
# explicit architecture decision instead of a silent edge.
#
# Usage (from the vectory root):
#   ruby scripts/check_dep_direction.rb                  # check the real graph
#   ruby scripts/check_dep_direction.rb --self-test      # negative tests
#   ruby scripts/check_dep_direction.rb --metadata-file FILE
#                                                        # check a captured JSON

require "json"
require "open3"

module DepDirection
  ROLE_SUMMARIES = {
    "model" => "a format model; models must not depend on transformers, engines, bindings, other models, or the VIM",
    "transformer" => "a transformer; it may depend only on vector-model and the models it bridges",
    "binding" => "a binding; it may depend only on the crates it surfaces",
  }.freeze

  RULES = {
    "vectory" => { role: "model", note: "the SVG format model" },
    "eikogram" => {
      role: "model",
      note: "the VIM (the Vector Image Model, the format-free drawing super-model; named eikogram by the owner)",
    },
    "eikogram-to-svg" => {
      role: "transformer",
      note: "the VIM -> SVG-model transformer; the only bridge between the drawing model and the SVG model",
      allowed: %w[vectory eikogram],
    },
    "emf-to-eikogram" => {
      role: "transformer",
      note: "the EMF-model -> VIM transformer; emf-core is an external pinned git dependency, not a workspace crate",
      allowed: %w[eikogram],
    },
    "emf88-parity" => {
      role: "binding",
      note: "the claricle/emf#88 byte-parity acceptance harness; binds the two transformers to compare final bytes against the Ruby renderer",
      allowed: %w[emf-to-eikogram eikogram-to-svg],
    },
    "vectory-ffi" => {
      role: "binding",
      note: "the C ABI over the SVG model for the Ruby gem's native engine (lane 3); surfaces the vectory crate only",
      allowed: %w[vectory],
    },
    "eikogram-ffi" => {
      role: "binding",
      note: "the C ABI over the EMF -> VIM -> SVG conversion for the Ruby gem's native render engine (claricle/emf#91); surfaces the two transformers; emf-core is an external pinned git dependency, not a workspace crate",
      allowed: %w[emf-to-eikogram eikogram-to-svg vectory],
    },
  }.freeze

  module_function

  # Returns the violation messages for the given workspace graph. `members` is a
  # list of crate names; `edges` is a list of [source, target, dep_kind] triples.
  def check(rules:, members:, edges:)
    violations = members.reject { |name| rules.key?(name) }.sort.map do |name|
      <<~MESSAGE
        UNCLASSIFIED CRATE: #{name}
          declare its role (model/transformer/binding) in RULES - adding a crate is an architecture decision
      MESSAGE
    end
    edges.sort.each do |source, target, kind|
      rule = rules[source]
      violations.concat(edge_violation(rule, source, target, kind)) if rule
    end
    violations
  end

  def edge_violation_message(rule, source, target, kind)
    allowed = Array(rule[:allowed])
    return if allowed.include?(target)

    summary = ROLE_SUMMARIES.fetch(rule[:role])
    detail = allowed.empty? ? summary : "#{summary}: #{allowed.join(', ')}"
    <<~MESSAGE
      ILLEGAL EDGE: #{source} --(#{kind})--> #{target}
        #{source} is #{detail}
    MESSAGE
  end

  def edge_violation(rule, source, target, kind)
    message = edge_violation_message(rule, source, target, kind)
    message ? [message] : []
  end

  # Builds [members, edges] from a `cargo metadata --format-version 1` document.
  # Only intra-workspace edges (path dependencies resolved to workspace members,
  # any dependency kind) are kept.
  def graph_from_metadata(metadata)
    members = workspace_members(metadata)
    edges = members.values.flat_map { |package| package_edges(members, package) }
    [members.keys, edges]
  end

  def workspace_members(metadata)
    packages = metadata.fetch("packages")
    metadata.fetch("workspace_members").to_h do |id|
      package = packages.find { |candidate| candidate.fetch("id") == id }
      raise KeyError, "workspace member #{id} not found among packages" unless package

      [package.fetch("name"), package]
    end
  end

  def package_edges(members, package)
    package.fetch("dependencies").filter_map do |dep|
      next unless members.key?(dep.fetch("name")) && dep["path"]

      [package.fetch("name"), dep.fetch("name"), dep["kind"] || "normal"]
    end
  end

  def cargo_metadata(workspace_root)
    # Capture stdout and stderr separately: cargo writes progress lines (e.g.
    # "Blocking waiting for file lock", "Updating crates.io index") to stderr.
    output, errors, status = Open3.capture3("cargo", "metadata", "--format-version", "1", chdir: workspace_root)
    abort("cargo metadata failed in #{workspace_root}:\n#{errors}#{output}") unless status.success?
    JSON.parse(output)
  end

  # Prints the verdict and returns the violations (empty list means the graph
  # is legal); the caller decides the exit status.
  def report(rules:, metadata:, label:)
    members, edges = graph_from_metadata(metadata)
    violations = check(rules: rules, members: members, edges: edges)
    if violations.empty?
      puts "dependency direction: #{members.length} crate(s), #{edges.length} workspace edge(s) checked - OK (#{label})"
    else
      puts "dependency direction: #{violations.length} violation(s) - FAILED (#{label})"
      violations.each { |violation| print violation }
    end
    violations
  end

  # Synthetic graphs exercising every rule arm of {check}; an empty
  # `expected` list asserts zero violations (the legal shape).
  def self_test_cases(rules)
    [
      { description: "the current graph shape is legal", members: %w[vectory], edges: [], expected: [] },
      { description: "the SVG model may not depend on the VIM - the transformer is the bridge",
        members: %w[vectory eikogram], edges: [%w[vectory eikogram normal]],
        expected: [/ILLEGAL EDGE: vectory --\(normal\)--> eikogram/] },
      { description: "the VIM may not depend on a format model",
        members: %w[vectory eikogram], edges: [%w[eikogram vectory normal]],
        expected: [/ILLEGAL EDGE: eikogram --\(normal\)--> vectory/] },
      { description: "an unclassified crate fails the check",
        members: %w[vectory svg-engine], edges: [], expected: [/UNCLASSIFIED CRATE: svg-engine/] },
    ].map { |case_data| case_data.merge(rules: rules) }
  end

  def self_test_case_passes?(violations, expected)
    missing = expected.reject { |pattern| violations.any? { |candidate| candidate.match?(pattern) } }
    unexpected = expected.empty? ? violations : []
    missing.empty? && unexpected.empty?
  end

  def self_test_failure(case_data, violations)
    puts "self-test FAIL: #{case_data.fetch(:description)}"
    puts "  expected: #{case_data.fetch(:expected).map(&:inspect).join(' / ')}"
    puts "  got: #{violations.map(&:inspect).join("\n       ")}"
  end

  # Runs the synthetic cases and returns the number of failures (zero means all
  # cases pass); the caller decides the exit status.
  def self_test(rules:)
    failures = 0
    self_test_cases(rules).each do |case_data|
      violations = check(rules: case_data.fetch(:rules), members: case_data.fetch(:members),
                         edges: case_data.fetch(:edges))
      if self_test_case_passes?(violations, case_data.fetch(:expected))
        puts "self-test PASS: #{case_data.fetch(:description)}"
      else
        failures += 1
        self_test_failure(case_data, violations)
      end
    end
    puts failures.zero? ? "self-test: all cases pass" : "self-test: #{failures} case(s) failed"
    failures
  end
end

if __FILE__ == $PROGRAM_NAME
  workspace_root = File.expand_path("../vectory-rs", __dir__)
  case ARGV[0]
  when nil
    violations = DepDirection.report(rules: DepDirection::RULES, metadata: DepDirection.cargo_metadata(workspace_root),
                                     label: "cargo metadata")
    exit(violations.empty? ? 0 : 1)
  when "--self-test"
    exit(DepDirection.self_test(rules: DepDirection::RULES).zero? ? 0 : 1)
  when "--metadata-file"
    abort("usage: #{__FILE__} --metadata-file FILE") unless ARGV[1]
    violations = DepDirection.report(rules: DepDirection::RULES, metadata: JSON.parse(File.read(ARGV[1])),
                                     label: "metadata file #{ARGV[1]}")
    exit(violations.empty? ? 0 : 1)
  else
    abort("usage: #{__FILE__} [--self-test | --metadata-file FILE]")
  end
end
