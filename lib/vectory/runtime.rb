# frozen_string_literal: true

require "leptris"

module Vectory
  # Picks the engine behind {Vectory::Runtime}'s dual-engine entry points:
  # the Rust SVG model shipped in this repository's vectory-rs workspace
  # (loaded through {Vectory::Native}), or the gem's own Ruby lane.
  #
  # `VECTORY_RUNTIME` selects the mode:
  #
  # - `auto` (default) — native when the vectory-ffi library loads, Ruby
  #   otherwise. A missing engine is the designed fallback, never an error;
  #   a PRESENT engine is always used, so a native failure surfaces as the
  #   native error's named Ruby class — never a silent switch.
  # - `ruby` — never touch the native engine; the Ruby lane answers even
  #   when the library is loadable.
  # - `native` — require it: a missing library raises
  #   {Runtime::EngineUnavailableError}, the named error. CI's native lane
  #   proves the Rust path.
  #
  # The class-level API (`SvgTransform`, `SvgTransformPipeline`, the SVG
  # writer) remains the Ruby lane untouched; {Runtime} is the dispatch the
  # ledger and dual-engine callers drive.
  module Runtime
    ENV_KEY = "VECTORY_RUNTIME"
    MODES = %w[auto ruby native].freeze

    # The serializer profiles with a Ruby lane in this gem. The emfsvg
    # profile is the renderer's serializer — native-only.
    RUBY_PROFILES = %i[ruby_xml].freeze

    # The native engine is demanded (or the operation has no Ruby lane) and
    # it is not loadable. Named, never a silent switch.
    class EngineUnavailableError < ::Vectory::Error; end

    class << self
      # @return [String] the validated mode
      # @raise [ArgumentError] when VECTORY_RUNTIME names no mode
      def mode
        value = ENV.fetch(ENV_KEY, nil)
        return "auto" if value.nil? || value.empty?
        return value if MODES.include?(value)

        raise ArgumentError,
              "#{ENV_KEY} must be one of #{MODES.join(', ')}, " \
              "got #{value.inspect}"
      end

      # Whether the native engine is loadable in this process.
      def native?
        !library.nil?
      end

      # The engine this process dispatches to: :native or :ruby.
      def engine
        selected = mode
        return :ruby if selected == "ruby"
        return :native if selected == "native"

        library.nil? ? :ruby : :native
      end

      # Decode + serialize through the selected engine.
      #
      # @param profile [:ruby_xml, :emfsvg_svg_builder] the serializer
      #   profile; `:ruby_xml` has the Ruby lane (the vectory writer's
      #   `Leptris::XML.parse(content).root.to_xml` bytes), `:emfsvg_svg_builder`
      #   is native-only.
      # @return [String] the serialized bytes
      # @raise [Runtime::EngineUnavailableError] when the operation has no
      #   usable engine
      def roundtrip(content, profile: :ruby_xml)
        if RUBY_PROFILES.include?(profile)
          return ruby_roundtrip(content) if engine == :ruby
        elsif mode == "ruby"
          raise EngineUnavailableError,
                "#{ENV_KEY}=ruby cannot run: the #{profile} serializer " \
                "profile is native-only (it has no Ruby lane in this gem)"
        end

        native_dispatch do |lib|
          lib.roundtrip(content, profile: profile)
        end
      end

      # Execute the transform pipeline described by the canonical step spec
      # ({Vectory::RuntimeSteps} — the shape both lanes interpret).
      #
      # @param steps [Array<Hash>] the canonical step spec
      # @return [String] the transformed bytes
      def pipeline(content, steps:)
        if engine == :ruby
          return RuntimeSteps.pipeline(steps).call(content)
        end

        native_dispatch do |lib|
          lib.pipeline(content, steps: steps)
        end
      end

      # The Native library, or nil. Probed once: an absent library means
      # "use Ruby" in `auto` mode.
      def library
        Native.library
      end

      # Forget the memoized probes (specs only).
      def reset!
        Native.reset!
      end

      private

      def native_dispatch
        lib = library
        return yield(lib) unless lib.nil?

        raise EngineUnavailableError,
              "#{ENV_KEY} demands the native engine and it is not loadable " \
              "(#{Native::LIBRARY_ENV} not set and no #{native_name} in " \
              "lib/vectory/native/ or under vectory-rs/target/)"
      end

      def native_name
        case RbConfig::CONFIG["host_os"]
        when /darwin/ then "libvectory_ffi.dylib"
        when /mswin|mingw/ then "vectory_ffi.dll"
        else "libvectory_ffi.so"
        end
      end

      # The Ruby lane: the vectory writer's bytes, the reference the golden
      # corpus pins (spec/vectory/rs_model_golden_spec.rb).
      def ruby_roundtrip(content)
        document = Leptris::XML.parse(content)
        raise ParsingError, "the document has no root" if document.root.nil?

        document.root.to_xml
      end
    end
  end
end
