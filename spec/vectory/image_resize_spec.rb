require "spec_helper"

RSpec.describe Vectory::ImageResize do
  let(:instance) { described_class.new }

  it "resizes images with missing or auto sizes" do
    image = Nokogiri::XML("<img src='spec/examples/19160-8.jpg'/>").root
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
    image["width"] = "20"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [20, 65]
    image.delete("width")
    image["height"] = "50"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [15, 50]
    image.delete("height")
    image["width"] = "500"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
    image.delete("width")
    image["height"] = "500"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
    image["width"] = "20"
    image["height"] = "auto"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [20, 65]
    image["width"] = "auto"
    image["height"] = "50"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [15, 50]
    image["width"] = "500"
    image["height"] = "auto"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
    image["width"] = "auto"
    image["height"] = "500"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
    image["width"] = "auto"
    image["height"] = "auto"
    expect(instance.call(image, "spec/examples/19160-8.jpg", 100, 100))
      .to eq [30, 100]
  end

  it "converts percentage sizes of images" do
    image = Nokogiri::XML("<img src='spec/examples/19160-8.jpg'/>").root
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[919, 3000], [919, 3000]]
    image["width"] = "20.4"
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[20, nil], [919, 3000]]
    image["height"] = "auto"
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[20, 0], [919, 3000]]
    image.delete("width")
    image["height"] = "20.4"
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[nil, 20], [919, 3000]]
    image["width"] = "auto"
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[0, 20], [919, 3000]]
    image["height"] = "30%"
    image["width"] = "50%"
    expect(instance.get_image_size(image, "spec/examples/19160-8.jpg"))
      .to eq [[459, 900], [919, 3000]]
  end

  it "resizes SVG with missing or auto sizes" do
    image = Nokogiri::XML(File.read("spec/examples/odf.svg")).root
    instance.call(image, "spec/examples/odf.svg", 100, 100)
    expect(image.attributes["viewBox"].value).to eq "0 0 102 102"
  end

  it "svg with missing sizes" do
    path = "spec/examples/bsi-1.svg"
    image = Nokogiri::XML(File.read(path)).root
    instance.call(image, path, 1200, 800)
  end

  it "xml with no size" do
    path = "spec/examples/img1.xml"
    content = File.read(path)
    data = Base64.strict_encode64(content)
    datauri = "data:application/xml;base64,#{data}"
    img_tag = %(<img src="#{datauri}" height="20" width="auto"/>)
    img = Nokogiri::XML(img_tag).root
    described_class.new.call(img, path, 1200, 800)
  end

  # Tests migrated from image_sizer_spec.rb
  describe "#image_size_percent" do
    context "when calculating percentages" do
      it "correctly calculates pixel value from percentage and real dimension" do
        # Using send to access private method
        expect(instance.send(:image_size_percent, 50.0, 200)).to eq(100.0)
      end

      it "returns nil if real_dimension is nil" do
        expect(instance.send(:image_size_percent, 50.0, nil)).to be_nil
      end

      it "returns nil if percentage_numeric_value is nil" do
        expect(instance.send(:image_size_percent, nil, 200)).to be_nil
      end

      it "handles floating point percentages" do
        expect(instance.send(:image_size_percent, 33.3,
                             300)).to be_within(0.01).of(99.9)
      end
    end
  end

  describe "#css_size_to_px" do
    let(:default_dpi) { 96 }
    let(:real_dimension_200) { 200 }

    context "with valid pixel units" do
      it "converts '100px' to 100" do
        expect(instance.send(:css_size_to_px, "100px", real_dimension_200,
                             dpi: default_dpi)).to eq(100)
      end

      it "converts '150.7px' to 150 (integer conversion)" do
        expect(instance.send(:css_size_to_px, "150.7px", real_dimension_200,
                             dpi: default_dpi)).to eq(150)
      end
    end

    context "with valid physical units and default DPI (96)" do
      it "converts '1in' to 96" do
        expect(instance.send(:css_size_to_px, "1in", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end

      it "converts '2.54cm' to 96" do
        # 2.54 cm * 96 dpi / 2.54 cm/in = 96
        expect(instance.send(:css_size_to_px, "2.54cm", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end

      it "converts '25.4mm' to 96" do
        # 25.4 mm * 96 dpi / 25.4 mm/in = 96
        expect(instance.send(:css_size_to_px, "25.4mm", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end

      it "converts '72pt' to 96" do
        # 72 pt * 96 dpi / 72 pt/in = 96
        expect(instance.send(:css_size_to_px, "72pt", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end

      it "converts '6pc' to 96" do
        # 6 pc * 12 pt/pc * 96 dpi / 72 pt/in = 96
        expect(instance.send(:css_size_to_px, "6pc", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end
    end

    context "with valid physical units and custom DPI" do
      let(:custom_dpi) { 300 }

      it "converts '1in' to 300 with 300 DPI" do
        expect(instance.send(:css_size_to_px, "1in", real_dimension_200,
                             dpi: custom_dpi)).to eq(300)
      end

      it "converts '1cm' to approx 118 with 300 DPI" do
        # 1 cm * 300 dpi / 2.54 cm/in = 118.11...
        expect(instance.send(:css_size_to_px, "1cm", real_dimension_200,
                             dpi: custom_dpi)).to eq(118)
      end
    end

    context "with percentage units" do
      it "converts '50%' of 200 to 100" do
        expect(instance.send(:css_size_to_px, "50%", real_dimension_200,
                             dpi: default_dpi)).to eq(100)
      end

      it "converts '25.5%' of 100 to 25 (integer conversion)" do
        # 100 * (25.5 / 100.0) = 25.5 -> 25.to_i
        expect(instance.send(:css_size_to_px, "25.5%", 100,
                             dpi: default_dpi)).to eq(25)
      end

      it "returns nil for '%' if real_dimension_for_percentage is nil" do
        expect(instance.send(:css_size_to_px, "50%", nil,
                             dpi: default_dpi)).to be_nil
      end
    end

    context "with plain numbers (interpreted as pixels)" do
      it "converts '150' to 150" do
        expect(instance.send(:css_size_to_px, "150", real_dimension_200,
                             dpi: default_dpi)).to eq(150)
      end

      it "converts '200.7' to 200 (integer conversion)" do
        expect(instance.send(:css_size_to_px, "200.7", real_dimension_200,
                             dpi: default_dpi)).to eq(200)
      end
    end

    context "with string variations" do
      it "handles leading/trailing spaces: ' 100px '" do
        expect(instance.send(:css_size_to_px, " 100px ", real_dimension_200,
                             dpi: default_dpi)).to eq(100)
      end

      it "handles case-insensitive units: '1IN'" do
        expect(instance.send(:css_size_to_px, "1IN", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end

      it "handles '2.54CM'" do
        expect(instance.send(:css_size_to_px, "2.54CM", real_dimension_200,
                             dpi: default_dpi)).to eq(96)
      end
    end

    context "with invalid or edge case inputs" do
      it "returns nil for nil input" do
        expect(instance.send(:css_size_to_px, nil, real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for empty string ''" do
        expect(instance.send(:css_size_to_px, "", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for 'auto'" do
        expect(instance.send(:css_size_to_px, "auto", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for unknown unit '10em'" do
        expect(instance.send(:css_size_to_px, "10em", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for malformed string 'abc'" do
        expect(instance.send(:css_size_to_px, "abc", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for malformed string '100 px' (space before unit)" do
        expect(instance.send(:css_size_to_px, "100 px", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "returns nil for malformed string 'px100'" do
        expect(instance.send(:css_size_to_px, "px100", real_dimension_200,
                             dpi: default_dpi)).to be_nil
      end

      it "parses negative values like '-10px' to -10" do
        expect(instance.send(:css_size_to_px, "-10px", real_dimension_200,
                             dpi: default_dpi)).to eq(-10)
      end
    end
  end

  describe "#image_size_interpret" do
    # We're testing the private method directly
    let(:default_dpi) { 96 }

    context "with basic pixel units" do
      it "interprets {'width' => '100px', 'height' => '50px'}" do
        img = { "width" => "100px", "height" => "50px" }
        realsize = [nil, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([100, 50])
      end
    end

    context "with physical units and default DPI" do
      it "interprets {'width' => '2in', 'height' => '1in'}" do
        img = { "width" => "2in", "height" => "1in" }
        realsize = [nil, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([192, 96])
      end

      it "interprets {'width' => '2.54cm', 'height' => '5.08cm'}" do
        img = { "width" => "2.54cm", "height" => "5.08cm" } # 1 inch, 2 inches
        realsize = [nil, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([96, 192])
      end
    end

    context "with percentage units" do
      it "interprets {'width' => '50%', 'height' => '25%'} with realsize [200, 400]" do
        img = { "width" => "50%", "height" => "25%" }
        realsize = [200, 400]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([100, 100])
      end

      it "interprets {'width' => '50%'} with realsize [200, nil]" do
        img = { "width" => "50%" }
        realsize = [200, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([100, nil])
      end

      it "interprets {'height' => '25%'} with realsize [nil, 400]" do
        img = { "height" => "25%" }
        realsize = [nil, 400]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([nil, 100])
      end

      it "interprets percentages with nil realsize" do
        img = { "width" => "50%", "height" => "25%" }
        expect(instance.send(:image_size_interpret, img, nil)).to eq([nil, nil])
        expect(instance.send(:image_size_interpret, img, [])).to eq([nil, nil])
      end
    end

    context "with mixed units" do
      it "interprets {'width' => '150px', 'height' => '50%'} with realsize [nil, 200]" do
        img = { "width" => "150px", "height" => "50%" }
        realsize = [nil, 200]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([150, 100])
      end
    end

    context "with missing width/height attributes" do
      it "interprets {'width' => '100px'}" do
        img = { "width" => "100px" }
        realsize = [nil, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([100, nil])
      end

      it "interprets {'height' => '50px'}" do
        img = { "height" => "50px" }
        realsize = [nil, nil]
        expect(instance.send(:image_size_interpret, img,
                             realsize)).to eq([nil, 50])
      end
    end
  end
end
