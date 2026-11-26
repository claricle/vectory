# frozen_string_literal: true

require_relative "lib/vectory/version"

Gem::Specification.new do |spec|
  spec.name          = "vectory"
  spec.version       = Vectory::VERSION
  spec.authors       = ["Ribose Inc."]
  spec.email         = ["open.source@ribose.com"]

  spec.summary       = "Pairwise vector image conversions."
  spec.description   = <<~DESCRIPTION
    Vectory performs pairwise vector image conversions for common
    vector image formats, such as SVG, EMF, EPS and PS, with PNG output via
    svgpng prebuilt native packages on arm64-darwin, x86_64-darwin, x86_64-linux,
    aarch64-linux, x86_64-linux-musl, aarch64-linux-musl and x64-mingw-ucrt. Its portable
    shell package preserves installation elsewhere and reports unsupported PNG output.
  DESCRIPTION

  spec.homepage      = "https://github.com/metanorma/vectory"
  spec.license       = "BSD-2-Clause"
  spec.metadata["rubygems_mfa_required"] = "true"

  spec.bindir        = "exe"
  spec.executables   = ["vectory"]
  spec.require_paths = ["lib"]
  spec.files         = `git ls-files -z`.split("\x0").reject do |f|
    f.match(%r{^(bin|spec|TODO\.[^/]+)/})
  end
  # Binaries are never tracked in git, and the ruby (source) gem stays
  # slim: the vectory-ffi cdylib is vendored ONLY into platform gems by
  # the cross-gem build (rakelib/native.rake's native:<platform> tasks,
  # which set VECTORY_VENDOR_CDYLIB=1), never by a plain `gem build`.
  spec.files.reject! do |f|
    f =~ /\.(dll|so|dylib)\Z/ &&
      !(ENV["VECTORY_VENDOR_CDYLIB"] == "1" &&
        f =~ %r{\Alib/vectory/native/(?:libvectory_ffi|vectory_ffi)\.(?:so|dylib|dll)\Z})
  end
  spec.required_ruby_version = ">= 3.1.0"

  spec.add_dependency "base64"
  spec.add_dependency "crass", "~> 1.0", ">= 1.0.7"
  spec.add_dependency "emfsvg", "~> 0.1", ">= 0.1.2"
  spec.add_dependency "image_size", ">= 3.2.0"
  spec.add_dependency "leptris", "~> 1.9.399"
  spec.add_dependency "marcel", "~> 1.0"
  spec.add_dependency "postsvg", "~> 0.3"
  spec.add_dependency "svgpng", "~> 0.1.0"
  spec.add_dependency "thor", "~> 1.0"
  spec.add_dependency "yeptris", "~> 0.6.56.1"
end
