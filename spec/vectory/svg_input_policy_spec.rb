# frozen_string_literal: true

require "spec_helper"
require "leptris"
require "socket"

RSpec.describe Vectory::SvgInputPolicy do
  let(:declaration) { '<!DOCTYPE svg [<!ENTITY secret SYSTEM "file:///secondary-secret">]>' }
  let(:source) { "#{declaration}<svg>&secret;</svg>" }

  it "rejects DTD bytes before the XML parser or any secondary access" do
    allow(Leptris::XML).to receive(:parse).and_call_original
    expect(Leptris::XML).not_to receive(:parse)
    expect(File).not_to receive(:read)
    expect(File).not_to receive(:open)
    expect(File).not_to receive(:binread)
    expect(Socket).not_to receive(:tcp)
    expect(TCPSocket).not_to receive(:new)
    expect { Vectory::Svg.from_content(source) }
      .to raise_error(Vectory::ForbiddenExternalReferenceError)
  end

  it "reads only the declared source path before rejecting its DTD" do
    Dir.mktmpdir do |directory|
      path = File.join(directory, "source.svg")
      File.binwrite(path, source)
      expect(File).to receive(:read).with(path, mode: "rb").once.and_call_original
      expect(File).not_to receive(:open)
      expect(File).not_to receive(:binread)
      expect(Leptris::XML).not_to receive(:parse)
      expect(Socket).not_to receive(:tcp)
      expect(TCPSocket).not_to receive(:new)
      expect { Vectory::Svg.from_path(path) }
        .to raise_error(Vectory::ForbiddenExternalReferenceError)
    end
  end

  it "rejects external and internal DTD declarations" do
    ['<!DOCTYPE svg SYSTEM "https://example.com/external.dtd">', "<!DOCTYPE svg>"].each do |dtd|
      expect { described_class.validate!("#{dtd}<svg/>") }
        .to raise_error(Vectory::ForbiddenExternalReferenceError)
    end
  end

  it "rejects NUL-padded UTF-16 and UTF-32 declarations before parsing" do
    %w[UTF-16LE UTF-16BE UTF-32LE UTF-32BE].each do |encoding|
      expect { Vectory::Svg.from_content(source.encode(encoding)) }
        .to raise_error(Vectory::ForbiddenExternalReferenceError)
    end
  end

  it "preserves literal declaration text in comments, CDATA and instructions" do
    safe = "<!-- #{declaration} --><?note #{declaration}?><svg><![CDATA[#{declaration}]]></svg>"
    expect { described_class.validate!(safe) }.not_to raise_error
    expect(Vectory::Svg.from_content(safe).content).to include("<![CDATA[")
  end

  it "continues scanning after multibyte comments and CDATA" do
    expect { described_class.validate!("<!-- é --><![CDATA[é]]>#{source}") }
      .to raise_error(Vectory::ForbiddenExternalReferenceError)
  end
end
