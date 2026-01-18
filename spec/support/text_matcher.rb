module Vectory
  class TextMatcher
    def initialize(allowed_changed_lines: 0,
                   allowed_changed_words_in_line: 0,
                   allowed_extra_lines_percent: 10)
      @allowed_changed_lines = allowed_changed_lines
      @allowed_changed_words_in_line = allowed_changed_words_in_line
      @allowed_extra_lines_percent = allowed_extra_lines_percent
    end

    def match?(expected, actual)
      expected_lines = expected.split("\n")
      actual_lines = actual.split("\n")

      # Allow actual to have more lines than expected (within a percentage tolerance)
      # This handles cases where different tool versions add extra output
      if expected_lines.count < actual_lines.count
        extra_lines = actual_lines.count - expected_lines.count
        allowed_extra = (expected_lines.count * @allowed_extra_lines_percent / 100.0).ceil

        if extra_lines > allowed_extra
          Vectory.ui.debug("Lines count differ by #{extra_lines} (allowed: #{allowed_extra}).")
          return false
        end

        Vectory.ui.debug("Actual has #{extra_lines} extra lines (within tolerance of #{allowed_extra}).")
        # Trim extra lines from actual for comparison
        actual_lines = actual_lines.first(expected_lines.count)
      end

      lines_the_same?(expected_lines, actual_lines)
    end

    private

    def lines_the_same?(expected_lines, actual_lines)
      results = []
      expected_lines
        .zip(actual_lines)
        .each_with_index do |(expected_line, actual_line), current_line|
          results[current_line] = analyze_line(expected_line, actual_line)
      end

      print_results(results)

      evaluate_results(results)
    end

    def analyze_line(expected, actual)
      expected_words = expected.split
      actual_words = actual.split

      padded_expected_words = pad_ary(expected_words, actual_words.count)
      padded_expected_words.zip(actual_words).count do |e, a|
        e != a
      end
    end

    def pad_ary(ary, target_length)
      ary.fill(nil, ary.length...target_length)
    end

    def print_results(results)
      results.each_with_index do |result, index|
        unless result.zero?
          Vectory.ui.debug("#{index}: #{result} different word(s).")
        end
      end
    end

    def evaluate_results(results)
      results.none? { |changed| changed >= @allowed_changed_words_in_line } &&
        results.count { |changed| changed > 0 } < @allowed_changed_lines
    end
  end
end
