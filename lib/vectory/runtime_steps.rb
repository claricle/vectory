# frozen_string_literal: true

module Vectory
  # The canonical step spec's Ruby interpretation — the same shape the
  # native engine reads over the FFI (vectory-rs/crates/vectory-ffi). One
  # interchange shape, two lanes:
  #
  #     {"name" => "graft_into",     "host_content" => "..."}
  #     {"name" => "disambiguate",   "index" => 1, "duplicate_ids" => {"r" => 2}}
  #     {"name" => "suffix_classes", "index" => 1}
  #     {"name" => "bind_namespaces","namespaces" => {"bind" => {...},
  #                                "unbind" => [...], "remap" => {...}}}
  #
  # String keys, JSON-shaped: it crosses the FFI as the pipeline spec
  # verbatim and builds this gem's transforms on the Ruby lane. Adding a
  # step means one entry in STEPS (and its FFI counterpart) — no dispatch
  # edit.
  module RuntimeSteps
    STEPS = {
      "graft_into" => ->(spec) {
        SvgTransform.graft_into(host_content: require_field(spec, "host_content"))
      },
      "disambiguate" => ->(spec) {
        SvgTransform.disambiguate(index: require_field(spec, "index"),
                                  duplicate_ids: spec.fetch("duplicate_ids", {}))
      },
      "suffix_classes" => ->(spec) {
        SvgTransform.suffix_classes(index: require_field(spec, "index"))
      },
      "bind_namespaces" => ->(spec) {
        namespaces = spec.fetch("namespaces", {})
        SvgTransform.bind_namespaces(
          bind: namespaces.fetch("bind", {}),
          unbind: namespaces.fetch("unbind", []),
          remap: namespaces.fetch("remap", {}),
        )
      },
    }.freeze

    private_constant :STEPS

    class << self
      # Builds one transform from a canonical spec entry.
      #
      # @raise [ArgumentError] on an unknown step or a missing required field
      def build(spec)
        name = spec.fetch("name")
        builder = STEPS.fetch(name) do
          raise ArgumentError, "unknown step name '#{name}'"
        end
        builder.call(spec)
      end

      # Builds the pipeline (the gem's own sequence validation applies).
      def pipeline(steps)
        SvgTransformPipeline.new(steps.map { |spec| build(spec) })
      end

      private

      def require_field(spec, field)
        spec.fetch(field) { raise ArgumentError, "step #{spec.fetch('name')} requires #{field}" }
      end
    end
  end
end
