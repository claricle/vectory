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
    vector image formats, such as SVG, EMF, EPS and PS.
  DESCRIPTION

  spec.homepage      = "https://github.com/metanorma/vectory"
  spec.license       = "BSD-2-Clause"

  spec.bindir        = "exe"
  spec.executables   = ["vectory"]
  spec.require_paths = ["lib"]
  spec.files         = `git ls-files -z`.split("\x0").reject do |f|
    f.match(%r{^(bin|spec)/})
  end
  spec.test_files = `git ls-files -- {spec}/*`.split("\n")
  spec.required_ruby_version = ">= 3.1.0"

  spec.add_dependency "base64"
  spec.add_dependency "emf2svg"
  spec.add_dependency "image_size", ">= 3.2.0"
  spec.add_dependency "marcel", "~> 1.0"
  spec.add_dependency "nokogiri", "~> 1.14"
  spec.add_dependency "thor", "~> 1.0"
  spec.add_dependency "ukiryu", "~> 0.2.4"
end
