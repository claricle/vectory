# frozen_string_literal: true

require "fiddle"
require "fiddle/struct"
require "json"
require "rbconfig"

module Vectory
  # Loads the vectory-ffi cdylib (the SVG model's native engine, lane 3) and
  # calls its two entry points.
  #
  # The shared library is looked up in this order:
  #   1. ENV["VECTORY_NATIVE_LIB"] — exact file path; when set, nothing else
  #      is searched.
  #   2. lib/vectory/native/ inside an installed platform gem (precompiled,
  #      no compile at install).
  #   3. <repo>/vectory-rs/target/release, then .../debug (a working
  #      checkout).
  #
  # A missing library is NOT an error: {Vectory::Runtime} falls back to the
  # Ruby lane in `auto` mode. A library that exists but fails to load raises
  # {NativeError} — a present-but-broken engine never silently switches
  # (the etassia#491 law).
  #
  # Binding decision (vectory-rs/crates/vectory-ffi/README.md): Fiddle +
  # cdylib with caller-allocated output buffers — zero runtime gem
  # dependencies, the svgpng-precedent convention.
  module Native
    LIBRARY_ENV = "VECTORY_NATIVE_LIB"
    REPO_ROOT = File.expand_path("../..", __dir__)
    PACKAGED_DIR = File.join(__dir__, "native").freeze
    SEARCH_DIRS = [
      PACKAGED_DIR,
      File.join(REPO_ROOT, "vectory-rs", "target", "release"),
      File.join(REPO_ROOT, "vectory-rs", "target", "debug"),
    ].freeze

    # Profile wire codes (vectory-ffi): the serializer profiles.
    PROFILE_RUBY_XML = 0
    PROFILE_EMFSVG_SVG_BUILDER = 1
    PROFILES = {
      ruby_xml: PROFILE_RUBY_XML,
      emfsvg_svg_builder: PROFILE_EMFSVG_SVG_BUILDER,
    }.freeze

    # The engine is demanded but nothing loadable exists. Named, never a
    # silent switch.
    class Unavailable < LoadError; end

    # The library exists but its ABI failed to load, or a call failed at the
    # ABI boundary (bad status, bad JSON body).
    class NativeError < ::Vectory::Error; end

    # Opens one cdylib and owns its three entry points.
    class Library
      # Mirrors `struct VectoryBuf` in vectory-rs/crates/vectory-ffi
      # (caller-allocated: pointer + length).
      BUF = Fiddle::CStructBuilder.create(Fiddle::CStruct,
                                          [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T],
                                          %w[ptr len]).freeze

      STATUS_OK = 0
      STATUS_INVALID_ARGUMENT = 1
      STATUS_PARSING = 2
      STATUS_TRANSFORM = 3

      # The JSON body's "class" names map to these Ruby classes.
      ERROR_CLASSES = {
        "ParsingError" => ::Vectory::ParsingError,
        "ArgumentError" => ArgumentError,
        "ConversionError" => ::Vectory::ConversionError,
        "ForbiddenExternalReferenceError" => ::Vectory::ForbiddenExternalReferenceError,
      }.freeze

      attr_reader :path

      def initialize(path)
        @path = path
        @handle = Fiddle::Handle.new(path, Fiddle::RTLD_NOW)
        @roundtrip = Fiddle::Function.new(@handle["vectory_v1_roundtrip"],
                                          [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T,
                                           Fiddle::TYPE_INT, Fiddle::TYPE_VOIDP],
                                          Fiddle::TYPE_INT)
        @pipeline = Fiddle::Function.new(@handle["vectory_v1_pipeline"],
                                         [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T,
                                          Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T,
                                          Fiddle::TYPE_VOIDP],
                                         Fiddle::TYPE_INT)
        @free = Fiddle::Function.new(@handle["vectory_v1_free"],
                                     [Fiddle::TYPE_VOIDP], Fiddle::TYPE_VOID)
      rescue Fiddle::DLError => e
        raise NativeError, "Cannot load the vectory native engine at #{path}: #{e.message}"
      end

      # Decode + serialize: XML bytes through one serializer profile.
      def roundtrip(content, profile: :ruby_xml)
        code = profile_code(profile)
        buffers([content]) do |pointers, out|
          copy(out, @roundtrip.call(pointers[0], pointers[0].size, code, out.to_ptr))
        end
      end

      # Execute the transform pipeline described by the canonical step spec
      # (the shape both lanes interpret; see lib/vectory/runtime_steps.rb).
      def pipeline(content, steps:)
        spec = JSON.generate({ "steps" => steps })
        buffers([content, spec]) do |pointers, out|
          copy(out, @pipeline.call(pointers[0], pointers[0].size,
                                   pointers[1], pointers[1].size, out.to_ptr))
        end
      end

      private

      def profile_code(profile)
        PROFILES.fetch(profile) do
          raise ArgumentError,
                "unknown native serializer profile #{profile.inspect}; " \
                "expected one of #{PROFILES.keys.join(', ')}"
        end
      end

      # One call owns its buffers until the output copy completes: the input
      # pointers and the output descriptor are released on every exit path,
      # with the native buffer freed first (the svgpng discipline).
      def buffers(contents)
        session = CallBuffers.new(contents)
        yield session.input_pointers, session.output
      ensure
        session&.release(@free)
      end

      # The allocations and their release order for one ABI call.
      class CallBuffers
        attr_reader :input_pointers, :output

        def initialize(contents)
          @owned = []
          @input_pointers = contents.map { |content| own_bytes(content) }
          @output = BUF.malloc(Fiddle::RUBY_FREE)
          @output.to_ptr[0, BUF.size] = "\0".b * BUF.size
        end

        # Native buffer first, then the Ruby-owned allocations.
        def release(free_function)
          free_function&.call(output.to_ptr)
          output.to_ptr.call_free
          @owned.each(&:call_free)
        end

        private

        def own_bytes(content)
          bytes = content.to_s.encode(Encoding::UTF_8).b
          pointer = Fiddle::Pointer.malloc([bytes.bytesize, 1].max, Fiddle::RUBY_FREE)
          pointer[0, bytes.bytesize] = bytes
          @owned << pointer
          pointer
        end
      end

      # Copies the result out, then maps a nonzero status (a JSON error body
      # naming the Ruby class) to the named error. Never falls back here:
      # error mapping is the caller's ({Vectory::Runtime}) decision.
      def copy(out, status)
        body = out.ptr.null? ? "".b : out.ptr[0, out.len].b
        return body if status == STATUS_OK

        raise_native_status(status, body)
      end

      def raise_native_status(status, body)
        info = parse_error_body(status, body)
        if info.is_a?(Hash)
          klass = ERROR_CLASSES.fetch(info["class"], NativeError)
          raise klass, info["message"].to_s
        end

        raise NativeError, "vectory native engine returned status #{status}: #{body}"
      end

      def parse_error_body(status, body)
        return nil unless [STATUS_INVALID_ARGUMENT, STATUS_PARSING, STATUS_TRANSFORM].include?(status)

        JSON.parse(body)
      rescue JSON::ParserError
        nil
      end
    end

    class << self
      # Path of the library that would be loaded, or nil.
      def library_path
        override = ENV.fetch(LIBRARY_ENV, nil)
        return (File.file?(override) ? override : nil) if override && !override.empty?

        SEARCH_DIRS.map { |dir| File.join(dir, library_filename) }.find { |path| File.file?(path) }
      end

      # Whether a loadable library exists in this process. A file that fails
      # to load raises; only a genuinely missing library answers false.
      def available?
        return false if library_path.nil?

        !library.nil?
      end

      # The memoized Library, or nil when no library file exists. Probed
      # once; the answer is process-lifetime (specs use reset!).
      def library
        return @library if instance_variable_defined?(:@library)

        path = library_path
        @library = path.nil? ? nil : Library.new(path)
      rescue Fiddle::DLError => e
        raise NativeError, "vectory native engine failed to load: #{e.message}"
      end

      # Forget the memoized probe (specs only).
      def reset!
        return unless instance_variable_defined?(:@library)

        remove_instance_variable(:@library)
      end

      private

      def library_filename
        case RbConfig::CONFIG["host_os"]
        when /darwin/ then "libvectory_ffi.dylib"
        when /mswin|mingw/ then "vectory_ffi.dll"
        else "libvectory_ffi.so"
        end
      end
    end
  end
end
