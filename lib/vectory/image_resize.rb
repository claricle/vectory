require "image_size"

module Vectory
  class ImageResize
    BORDER_WIDTH = 2

    def call(img, path, maxheight, maxwidth)
      s, realsize = get_image_size(img, path)
      # Ensure s[0] and s[1] are not nil before setting viewBox
      img.name == "svg" && !img["viewBox"] && s[0] && s[1] and
        img["viewBox"] = viewbox(s)
      s, skip = image_dont_resize(s, realsize)
      skip and return s
      s = image_size_fillin(s, realsize)
      image_shrink(s, maxheight, maxwidth)
    end

    def get_image_size(img, path)
      detected = detect_size(path)
      realsize = detected if detected
      s = image_size_interpret(img, realsize || [nil, nil])
      # No call to image_size_zeroes_complete here
      [s, realsize] # s should now be correctly populated
    end

    private

    def detect_size(path)
      size = ImageSize.path(path).size
      return unless size
      return if size.all? { |x| x.nil? || x.zero? }

      size
    end

    def viewbox(dimensions)
      width = dimensions[0] + BORDER_WIDTH
      height = dimensions[1] + BORDER_WIDTH

      "0 0 #{width} #{height}"
    end

    def image_dont_resize(dim, realsize)
      dim.nil? and return [[nil, nil], true]
      realsize.nil? and return [dim, true]
      dim[0] == nil && dim[1] == nil and return [dim, true]
      [dim, false]
    end

    def image_size_fillin(dim, realsize)
      dim[1] = fill_size(dim[1], dim[0], realsize[1], realsize[0])
      dim[0] = fill_size(dim[0], dim[1], realsize[0], realsize[1])

      dim
    end

    def fill_size(current, another, real_current, real_another)
      # Ensure 'current' and 'another' are not nil before calling zero?
      # Also ensure real_another is not nil or zero to prevent division by zero
      return current unless (current.nil? || current.zero?) &&
        !(another.nil? || another.zero?) &&
        !(real_another.nil? || real_another.zero?)

      another * real_current / real_another
    end

    def image_shrink(dim, maxheight, maxwidth)
      dim[1] > maxheight and
        dim = [(dim[0] * maxheight / dim[1]).ceil, maxheight]
      dim[0] > maxwidth and
        dim = [maxwidth, (dim[1] * maxwidth / dim[0]).ceil]
      dim
    end

    def image_size_interpret(img, realsize)
      # Extract parent dimensions for percentage calculations
      parent_w_px = realsize.is_a?(Array) && realsize.length.positive? ? realsize[0] : nil
      parent_h_px = realsize.is_a?(Array) && realsize.length > 1 ? realsize[1] : nil

      # Process width and height using the original css_size_to_px signature
      w = css_size_to_px(img["width"], parent_w_px)
      h = css_size_to_px(img["height"], parent_h_px)

      # If attributes resulted in no dimensions, but realsize exists, use realsize.
      # This brings back some of the logic from the removed image_size_zeroes_complete.
      if w.nil? && h.nil? && realsize && !(realsize[0].nil? && realsize[1].nil?)
        w = realsize[0]
        h = realsize[1]
      end

      # If a dimension attribute led to a nil value (e.g. "auto", or "%" of nil parent)
      # and the other dimension is resolved, default the nil dimension to 0.
      if img["width"] && w.nil? && !h.nil?
        w = 0
      end
      if img["height"] && h.nil? && !w.nil?
        h = 0
      end

      [w, h]
    end

    # Enhanced version of image_size_percent that handles percentage calculations
    # This is now a helper method for css_size_to_px
    def image_size_percent(percentage_numeric_value, real_dimension)
      return nil if real_dimension.nil? || percentage_numeric_value.nil?

      # Calculates the dimension in pixels. The result is a float.
      (real_dimension * (percentage_numeric_value / 100.0))
    end

    # New method from image_sizer.rb, incorporated directly into ImageResize
    # Converts CSS length string to pixels.
    def css_size_to_px(size_str, real_dimension_for_percentage, dpi: 96)
      return nil unless size_str.is_a?(String)

      cleaned_size_str = size_str.strip.downcase

      # Handle keywords like "auto" which don't resolve to a numeric pixel value.
      return nil if cleaned_size_str == "auto"

      # Handle plain numbers (e.g., "150") as pixels.
      if cleaned_size_str.match?(/^\d+(\.\d+)?$/)
        return cleaned_size_str.to_f.to_i
      end

      # Match numeric value and unit (e.g., "10.5cm", "50%", "-5px").
      match = cleaned_size_str.match(/^([-+]?[0-9]*\.?[0-9]+)([a-z%]+)$/)
      unless match
        # Not a plain number and not in "valueUnit" format
        return nil
      end

      value = match[1].to_f
      unit = match[2]
      px_value = nil # This will store the calculated pixel value as a float.

      case unit
      when "px"
        px_value = value
      when "in"
        px_value = value * dpi
      when "cm"
        px_value = value * dpi / 2.54
      when "mm"
        px_value = value * dpi / 25.4
      when "pt" # 1 point = 1/72 inch
        px_value = value * dpi / 72.0
      when "pc" # 1 pica = 12 points
        px_value = value * 12.0 * dpi / 72.0
      when "%"
        # Use the enhanced image_size_percent method
        px_value = image_size_percent(value, real_dimension_for_percentage)
      else
        return nil # Unknown unit
      end

      return nil if px_value.nil?

      # For 'px' and '%' units, truncate to match "integer conversion" expectation.
      # For physical units, round to handle floating point inaccuracies correctly.
      if ["px", "%"].include?(unit)
        px_value.to_i
      else
        px_value.round
      end
    end

    # REMOVE the image_size_zeroes_complete method entirely if it's still present
    # def image_size_zeroes_complete(dim, realsize)
    #   ...
    # end
  end
end
