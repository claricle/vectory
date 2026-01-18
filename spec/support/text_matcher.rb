module Vectory
  class TextMatcher
    def initialize(allowed_changed_lines: 0,
                   allowed_changed_words_in_line: 0)
      @allowed_changed_lines = allowed_changed_lines
      @allowed_changed_words_in_line = allowed_changed_words_in_line
    end

    def match?(expected, actual)
      expected_lines = expected.split("\n")
      actual_lines = actual.split("\n")

      if expected_lines.count < actual_lines.count
        Vectory.ui.debug("Lines count differ.")
        return false
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
