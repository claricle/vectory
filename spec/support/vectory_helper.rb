require "nokogiri"
require "tmpdir"

module Vectory
  module Helper
    def with_tmp_dir(&block)
      Dir.mktmpdir(nil, Vectory.root_path.join("tmp/"), &block)
    end

    def in_tmp_dir(&block)
      Dir.mktmpdir(nil, Vectory.root_path.join("tmp/")) do |dir|
        Dir.chdir(dir, &block)
      end
    end

    # Skip tests that rely on Inkscape EMF export on Windows
    # Inkscape 1.4.x on Windows has a bug where EMF export returns
    # exit code 0 but doesn't create the output file.
    def skip_emf_on_windows
      if windows_ci?
        skip "Inkscape EMF export is broken on Windows (returns exit 0 but no output file)"
      end
    end

    # Check if running on Windows CI
    def windows_ci?
      ENV["CI"] && Gem.win_platform?
    end

    # Check if running on Windows (any environment)
    def windows?
      Gem.win_platform?
    end
  end
end
