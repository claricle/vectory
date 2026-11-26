# frozen_string_literal: true

require "rb_sys/toolchain_info"
require "rubygems/package"

# The vendored-cdylib skeleton (lane 3, claricle/vectory#99; the
# etassia-rs machinery). The gem builds exactly the way rb-sys-dock drives
# it — `rake native:$RUBY_TARGET gem` inside the cross container. Each
# native:<platform> task vendors the vectory-ffi cdylib for that triple
# into lib/vectory/native/ inside the rake process — triple from rb_sys's
# own platform table, cargo build, cp, then declare the file on the spec
# and package it. The Ruby (source) gem never carries binaries: the
# gemspec rejects them unless VECTORY_VENDOR_CDYLIB=1, which only these
# tasks set the vendored file under.
PLATFORM_GEMS = %w[
  x86_64-linux
  aarch64-linux
  x86_64-darwin
  arm64-darwin
  x64-mingw-ucrt
].freeze

REPO_ROOT = File.expand_path("..", __dir__) # the repository root (rakelib/..)
RUST_ROOT = File.join(REPO_ROOT, "vectory-rs") # the in-repo Rust workspace
FFI_BASENAME = "vectory_ffi"

def vendor_cdylib(plat)
  triple = RbSys::ToolchainInfo::DATA.fetch(plat).fetch("rust-target")
  target_dir = ENV["CARGO_TARGET_DIR"] || File.join(RUST_ROOT, "target")

  sh "cargo", "build", "--release", "-p", "vectory-ffi", "--target", triple, chdir: RUST_ROOT
  art = Dir[File.join(target_dir, "#{triple}/release/lib#{FFI_BASENAME}.{so,dylib}"),
            File.join(target_dir, "#{triple}/release/#{FFI_BASENAME}.dll")].first
  raise "cdylib artifact not found for #{plat} (#{triple})" unless art

  mkdir_p "lib/vectory/native"
  cp art, "lib/vectory/native/"
  File.join("lib/vectory/native", File.basename(art))
end

namespace :native do
  desc "Build the host cdylib and smoke one corpus SVG natively (both lanes)"
  task :smoke do
    sh "cargo", "build", "--release", "-p", "vectory-ffi", chdir: RUST_ROOT
    dylib = Dir[File.join(RUST_ROOT, "target/release/libvectory_ffi.{so,dylib}"),
                File.join(RUST_ROOT, "target/release/vectory_ffi.dll")].first
    raise "host cdylib artifact not found" unless dylib

    ruby File.join("scripts", "smoke_native.rb"),
         dylib,
         File.join("spec", "fixtures", "svg_transform", "pipeline_inputs", "selected.svg"),
         File.join("spec", "fixtures", "svg_transform", "pipeline_inputs", "selected.svg")
  end

  PLATFORM_GEMS.each do |plat|
    desc "Vendor the #{plat} cdylib and build the #{plat} platform gem"
    task plat do
      vendored = vendor_cdylib(plat)

      spec = Gem::Specification.load("vectory.gemspec")
      raise "vectory.gemspec failed to load" if spec.nil?

      spec.platform = plat
      spec.files << vendored
      ENV["VECTORY_VENDOR_CDYLIB"] = "1"
      gem_file = Gem::Package.build(spec)
      mkdir_p "pkg"
      mv gem_file, "pkg/"
      puts "Built pkg/#{File.basename(gem_file)}"
    end
  end
end
