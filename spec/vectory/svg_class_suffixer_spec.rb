# frozen_string_literal: true

require "spec_helper"
require "vectory/svg_class_suffixer"

RSpec.describe Vectory::SvgClassSuffixer, :aggregate_failures do
  let(:suffix) { "_inject_0" }
  let(:input) { File.read("spec/fixtures/svg_transform/class_suffixer/styles-input.svg") }
  let(:expected) { File.read("spec/fixtures/svg_transform/class_suffixer/styles-output.svg") }
  let(:output) { described_class.rewrite(input, suffix) }
  let(:document) { Leptris::XML.parse(output) }

  it "suffixes compound, comma, functional and deeply nested selectors" do
    expect(output).to be_xml_equivalent_to(expected)
  end

  it "preserves every byte of declarations and unrelated selector content" do
    actual_css = document.xpath("//*[local-name()='style']").first.text
    expected_css = Leptris::XML.parse(expected).xpath("//*[local-name()='style']").first.text
    expect(actual_css).to eq(expected_css)
    expect(document.xpath("//*[@id='rect1']").first["style"]).to eq("fill: url(#grad1);")
  end

  it "is idempotent for XML class lists and nested CSS" do
    expect(described_class.rewrite(output, suffix)).to be_xml_equivalent_to(output)
  end

  [
    [".a{}<!-- c -->.b{}", ".a_inject_0{}<!-- c -->.b_inject_0{}"],
    [".fo<!-- keep -->o{}", ".fo<!-- keep -->o_inject_0{}"],
    ['.a{content:"<!-- keep -->.b"}.c{}', '.a_inject_0{content:"<!-- keep -->.b"}.c_inject_0{}'],
    ["<![CDATA[.a]]><!-- keep -->{}<![CDATA[.b{}]]>", "<![CDATA[.a_inject_0]]><!-- keep -->{}<![CDATA[.b_inject_0{}]]>"],
  ].each do |css, expected_css|
    it "preserves XML child nodes while suffixing #{css}" do
      source = %(<svg xmlns="http://www.w3.org/2000/svg"><style>#{css}</style></svg>)
      result = described_class.rewrite(source, suffix)
      style = Nokogiri::XML(result, &:strict).at_xpath("//*[local-name()='style']")
      expect(style.children.map(&:to_xml).join).to eq(expected_css)
      expect(described_class.rewrite(result, suffix)).to eq(result)
    end
  end

  context "with scoped rules" do
    let(:input) do
      "<svg><style>@media screen { @scope (.box:is(.active)) to (.stop) { " \
        '.foo { content: ".box" } } }</style><g class="box active"><path class="foo stop"/></g></svg>'
    end

    it "suffixes scope roots, limits and nested rules without changing declarations" do
      expect(document.xpath("//style").first.text).to eq(
        "@media screen { @scope (.box_inject_0:is(.active_inject_0)) to (.stop_inject_0) { " \
        '.foo_inject_0 { content: ".box" } } }',
      )
      expect(document.xpath("//*[@class]").map { |node| node["class"] })
        .to eq(["box_inject_0 active_inject_0", "foo_inject_0 stop_inject_0"])
    end

    it "does not suffix scope selectors again on repeat" do
      expect(described_class.rewrite(output, suffix)).to be_xml_equivalent_to(output)
    end
  end

  context "with escaped identifiers" do
    let(:input) { File.read("spec/fixtures/svg_transform/class_suffixer/escaped-input.svg") }
    let(:expected) { File.read("spec/fixtures/svg_transform/class_suffixer/escaped-output.svg") }

    it "preserves escape spelling while suffixing the decoded matching class" do
      expect(output).to be_xml_equivalent_to(expected)
      expect(document.xpath("//*[local-name()='style']").first.text)
        .to eq('.foo\+bar_inject_0, .\66 oo\+bar_inject_0 { fill: red; }')
      expect(document.xpath("//*[local-name()='path']").first["class"])
        .to eq("foo+bar_inject_0")
    end

    it "matches the repeat golden without adding another suffix" do
      repeated = described_class.rewrite(output, suffix)
      golden = File.read("spec/fixtures/svg_transform/class_suffixer/escaped-repeat.svg")
      expect(repeated).to be_xml_equivalent_to(golden)
    end
  end

  context "with CSS nesting inside style rules" do
    let(:input) do
      "<svg><style>.a{fill:red; .b{fill:blue}} .c{ &amp; .d {x:y} color:blue; " \
        '@media screen { color:red; > .e:is(.f) {content:".g"; .g {x:y}} } ' \
        '--theme: { .untouched {x:y} }; --value: .untouched; div.h:hover {x:y}}</style><g class="a b c d e f g h"/></svg>'
    end

    it "rewrites nested selectors while leaving declarations and custom properties intact" do
      expect(document.xpath("//style").first.text).to eq(
        ".a_inject_0{fill:red; .b_inject_0{fill:blue}} .c_inject_0{ & .d_inject_0 {x:y} color:blue; " \
        '@media screen { color:red; > .e_inject_0:is(.f_inject_0) {content:".g"; .g_inject_0 {x:y}} } ' \
        "--theme: { .untouched {x:y} }; --value: .untouched; div.h_inject_0:hover {x:y}}",
      )
      expect(document.xpath("//g").first["class"]).to eq(("a".."h").map { |name| "#{name}#{suffix}" }.join(" "))
    end

    it "is stable on repeat" do
      expect(described_class.rewrite(output, suffix)).to be_xml_equivalent_to(output)
    end
  end

  context "with a suffix starting with a hex digit" do
    let(:suffix) { "b" }
    let(:input) do
      '<svg><style>.foo\31, .foo\000031, .foo\31 , .foo\\\\31 {x:y}</style>' \
        '<g class="foo1 foo\\31"/></svg>'
    end

    it "keeps escaped identifiers semantically equal to the suffixed XML classes" do
      css = document.xpath("//style").first.text
      identifiers = Crass.parse(css).first[:selector][:tokens].select { |token| token[:node] == :ident }
      expect(identifiers.map { |token| token[:value] }).to eq(["foo1b", "foo1b", "foo1b", 'foo\\31b'])
      expect(document.xpath("//g").first["class"]).to eq('foo1b foo\\31b')
      expect(css).to eq('.foo\31 b, .foo\000031b, .foo\31 b, .foo\\\\31b {x:y}')
    end

    it "does not append the suffix again" do
      expect(described_class.rewrite(output, suffix)).to be_xml_equivalent_to(output)
    end
  end

  context "with multiple SVGs in a host document" do
    let(:input) do
      '<host class="host"><style>.host { fill: red }</style>' \
        '<svg class="root"><g class="a  b"><svg><path class="a"/></svg></g></svg>' \
        '<svg xmlns="http://www.w3.org/2000/svg"><style>.a { x: 1 }</style><path class="a"/></svg></host>'
    end

    it "rewrites all SVG roots and descendants once and preserves host content" do
      classes = document.xpath("//*[@class]").map { |element| element["class"] }
      expect(classes).to eq(["host", "root_inject_0", "a_inject_0  b_inject_0", "a_inject_0", "a_inject_0"])
      expect(document.xpath("//host/style").first.text).to eq(".host { fill: red }")
    end
  end

  context "with empty values" do
    let(:input) { '<svg class=""><style/><g class=" "/><path/></svg>' }

    it "preserves empty classes, empty styles and absent classes" do
      expect(output).to be_xml_equivalent_to(input)
    end
  end

  context "with malformed CSS" do
    let(:input) do
      '<svg class="foo"><style>.foo { broken; content: &quot;.foo&quot;; } .unfinished</style></svg>'
    end

    it "rewrites recognized selectors and preserves discarded incomplete streams" do
      expect(document.xpath("//style").first.text)
        .to eq('.foo_inject_0 { broken; content: ".foo"; } .unfinished')
    end
  end

  it "keeps an unclosed block unclosed while rewriting its known selector" do
    result = described_class.rewrite("<svg><style>.foo { fill: red</style></svg>", suffix)
    expect(Leptris::XML.parse(result).xpath("//style").first.text).to eq(".foo_inject_0 { fill: red")
  end

  it "preserves unknown at-rules without guessing whether their blocks contain selectors" do
    result = described_class.rewrite("<svg><style>@custom { value: .foo; }</style></svg>", suffix)
    expect(Leptris::XML.parse(result).xpath("//style").first.text).to eq("@custom { value: .foo; }")
  end

  it "detects a suffix represented by CSS escapes from its semantic value" do
    result = described_class.rewrite('<svg><style>.foo_inject_\30 { x: 1 }</style></svg>', suffix)
    expect(Leptris::XML.parse(result).xpath("//style").first.text).to eq('.foo_inject_\30 { x: 1 }')
  end

  it "preserves Unicode identifiers and comments between class-selector tokens" do
    result = described_class.rewrite('<svg><style>/*é*/ ./**/café { content: ".café" }</style></svg>', suffix)
    expect(Leptris::XML.parse(result).xpath("//style").first.text)
      .to eq('/*é*/ ./**/café_inject_0 { content: ".café" }')
  end

  it "rejects invalid XML" do
    expect { described_class.rewrite("<svg><path></svg>", suffix) }.to raise_error(Leptris::XML::ParseError)
  end

  it "preserves CSS carriage returns while mapping Crass token positions" do
    result = described_class.rewrite('<svg><style>/*x*/&#13;\n.foo { x: 1 }</style></svg>'.sub('\\n', "\n"), suffix)
    expect(Leptris::XML.parse(result).xpath("//style").first.text).to eq("/*x*/\r\n.foo_inject_0 { x: 1 }")
  end
end
