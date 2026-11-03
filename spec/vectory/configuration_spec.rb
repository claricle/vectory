# frozen_string_literal: true

require "spec_helper"

RSpec.describe Vectory::Configuration do
  let(:fixtures) { File.expand_path("../fixtures/configuration", __dir__) }

  before { described_class.reset! }
  after { described_class.reset! }

  describe ".load_from_file" do
    it "loads every key from a full config" do
      config = described_class.load_from_file(File.join(fixtures, "full.yml"))

      expect(config).to be(described_class.instance)
      expect(config.to_h).to eq(
        timeout: 60,
        cache_ttl: 900,
        cache_enabled: true,
        temp_dir: "/tmp/vectory",
        verbose_logging: true,
      )
    end

    it "falls back to defaults for missing keys" do
      config = described_class.load_from_file(File.join(fixtures,
                                                        "partial.yml"))

      expect(config.to_h).to eq(
        timeout: 30,
        cache_ttl: described_class::DEFAULT_CACHE_TTL,
        cache_enabled: true,
        temp_dir: nil,
        verbose_logging: false,
      )
    end

    it "keeps explicit false booleans, numeric strings, and zero values" do
      described_class.instance.verbose_logging = true
      config = described_class.load_from_file(File.join(fixtures,
                                                        "false_booleans.yml"))

      expect(config.to_h).to eq(
        timeout: "045",
        cache_ttl: 0,
        cache_enabled: false,
        temp_dir: nil,
        verbose_logging: false,
      )
    end

    it "keeps the previous temp_dir when the file has a null temp_dir" do
      described_class.instance.temp_dir = "/previous"
      config = described_class.load_from_file(File.join(fixtures,
                                                        "false_booleans.yml"))

      expect(config.temp_dir).to eq("/previous")
    end

    it "raises NoMethodError for an empty file (current behaviour)" do
      expect do
        described_class.load_from_file(File.join(fixtures, "empty.yml"))
      end.to raise_error(NoMethodError)
    end

    it "reads the file through Yeptris::Psych" do
      path = File.join(fixtures, "full.yml")
      expect(Yeptris::Psych).to receive(:load_file).with(path).and_call_original

      described_class.load_from_file(path)
    end

    it "raises Errno::ENOENT for a missing path" do
      expect do
        described_class.load_from_file(File.join(fixtures, "missing.yml"))
      end.to raise_error(Errno::ENOENT)
    end
  end
end
