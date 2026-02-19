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
  end
end
