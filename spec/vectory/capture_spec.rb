require "spec_helper"

RSpec.describe Vectory::Capture do
  describe ".with_timeout" do
    context "with successful command execution" do
      it "captures stdout" do
        result = described_class.with_timeout("echo", "hello world")

        # Windows echo adds quotes, Unix doesn't
        expected = Gem.win_platform? ? '"hello world"' : "hello world"
        expect(result[:stdout].strip).to eq(expected)
        expect(result[:stderr]).to be_empty
        expect(result[:status].success?).to be true
        expect(result[:timeout]).to be false
        expect(result[:pid]).to be_a(Integer)
      end

      it "captures stderr" do
        # Use a command that writes to stderr across platforms
        result = if Gem.win_platform?
                   # On Windows, use cmd to write to stderr
                   described_class.with_timeout(
                     "cmd", "/c", "echo error message 1>&2"
                   )
                 else
                   # On Unix, use sh to write to stderr
                   described_class.with_timeout(
                     "sh", "-c", "echo 'error message' >&2"
                   )
                 end

        expect(result[:stderr].strip).to eq("error message")
        expect(result[:status].success?).to be true
        expect(result[:timeout]).to be false
      end

      it "handles both stdout and stderr" do
        result = if Gem.win_platform?
                   described_class.with_timeout(
                     "cmd", "/c", "echo out && echo err 1>&2"
                   )
                 else
                   described_class.with_timeout(
                     "sh", "-c", "echo 'out'; echo 'err' >&2"
                   )
                 end

        expect(result[:stdout].strip).to eq("out")
        expect(result[:stderr].strip).to eq("err")
        expect(result[:status].success?).to be true
      end

      it "works with command arrays" do
        result = described_class.with_timeout("echo", "test")

        expect(result[:stdout].strip).to eq("test")
        expect(result[:status].success?).to be true
      end
    end

    context "with command failure" do
      it "captures exit status" do
        result = if Gem.win_platform?
                   described_class.with_timeout("cmd", "/c", "exit 42")
                 else
                   described_class.with_timeout("sh", "-c", "exit 42")
                 end

        expect(result[:status].success?).to be false
        expect(result[:status].exitstatus).to eq(42)
        expect(result[:timeout]).to be false
      end
    end

    context "with stdin_data" do
      it "sends input to command" do
        result = if Gem.win_platform?
                   # Windows: use findstr to read from stdin (simple cmd command)
                   described_class.with_timeout(
                     "cmd", "/c", "findstr /C:test",
                     stdin_data: "test input"
                   )
                 else
                   # Unix: use cat to read from stdin
                   described_class.with_timeout(
                     "cat",
                     stdin_data: "test input",
                   )
                 end

        expect(result[:stdout].strip).to eq("test input")
        expect(result[:status].success?).to be true
      end
    end

    context "with timeout" do
      it "terminates long-running command" do
        skip "Timeout tests not reliable on Windows due to command spawning behavior" if Gem.win_platform?

        result = described_class.with_timeout(
          "sleep", "10",
          timeout: 1
        )

        expect(result[:timeout]).to be true
      end

      it "does not timeout fast commands" do
        result = described_class.with_timeout(
          "echo", "quick",
          timeout: 5
        )

        expect(result[:timeout]).to be false
        expect(result[:stdout].strip).to eq("quick")
        expect(result[:status].success?).to be true
      end
    end

    context "with binary mode" do
      it "handles binary data" do
        skip "Binary data handling via PowerShell needs investigation on Windows" if Gem.win_platform?

        binary_data = "\x00\x01\x02\xFF".b

        result = described_class.with_timeout(
          "cat",
          stdin_data: binary_data,
          binmode: true,
        )

        expect(result[:stdout]).to eq(binary_data)
        expect(result[:status].success?).to be true
      end
    end

    context "with environment variables" do
      it "passes environment to command" do
        result = if Gem.win_platform?
                   described_class.with_timeout(
                     { "TEST_VAR" => "test_value" },
                     "cmd", "/c", "echo %TEST_VAR%"
                   )
                 else
                   described_class.with_timeout(
                     { "TEST_VAR" => "test_value" },
                     "sh", "-c", "echo $TEST_VAR"
                   )
                 end

        expect(result[:stdout].strip).to eq("test_value")
        expect(result[:status].success?).to be true
      end
    end

    context "with different working directories" do
      it "executes command in specified directory" do
        skip "Working directory test needs investigation on Windows" if Gem.win_platform?

        temp_dir = Dir.mktmpdir

        begin
          result = described_class.with_timeout(
            "pwd",
            chdir: temp_dir,
          )
          # On macOS, /var is a symlink to /private/var, so normalize paths
          actual_dir = File.realpath(result[:stdout].strip)
          expected_dir = File.realpath(temp_dir)
          expect(actual_dir).to eq(expected_dir)

          expect(result[:status].success?).to be true
        ensure
          FileUtils.rm_rf(temp_dir)
        end
      end
    end

    context "with signal handling" do
      it "uses custom signal on timeout" do
        skip "Timeout tests not reliable on Windows due to command spawning behavior" if Gem.win_platform?

        result = described_class.with_timeout(
          "sleep", "10",
          timeout: 1,
          signal: :TERM
        )

        expect(result[:timeout]).to be true
      end
    end

    context "edge cases" do
      it "handles commands that exit immediately" do
        result = if Gem.win_platform?
                   described_class.with_timeout("cmd", "/c", "exit 0")
                 else
                   described_class.with_timeout("true")
                 end

        expect(result[:status].success?).to be true
        expect(result[:timeout]).to be false
      end

      it "handles empty output" do
        result = if Gem.win_platform?
                   described_class.with_timeout("cmd", "/c", "")
                 else
                   described_class.with_timeout("true")
                 end

        expect(result[:stdout]).to be_empty
        expect(result[:stderr]).to be_empty
        expect(result[:status].success?).to be true
      end

      it "handles large output" do
        # Generate large output (platform-specific)
        result = if Gem.win_platform?
                   described_class.with_timeout(
                     "cmd", "/c", "for /L %i in (1,1,1000) do @echo %i"
                   )
                 else
                   described_class.with_timeout(
                     "seq", "1", "1000"
                   )
                 end

        lines = result[:stdout].split("\n").map(&:strip).reject(&:empty?)
        expect(lines.size).to be >= 1000
        expect(result[:status].success?).to be true
      end
    end
  end
end
