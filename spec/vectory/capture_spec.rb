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
        if Gem.win_platform?
          # On Windows, use PowerShell to write to stderr
          result = described_class.with_timeout(
            "powershell", "-Command",
            "[Console]::Error.WriteLine('error message')"
          )
        else
          # On Unix, use sh to write to stderr
          result = described_class.with_timeout(
            "sh", "-c", "echo 'error message' >&2"
          )
        end

        expect(result[:stderr].strip).to eq("error message")
        expect(result[:status].success?).to be true
        expect(result[:timeout]).to be false
      end

      it "handles both stdout and stderr" do
        if Gem.win_platform?
          result = described_class.with_timeout(
            "powershell", "-Command",
            "Write-Output 'out'; [Console]::Error.WriteLine('err')"
          )
        else
          result = described_class.with_timeout(
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
        if Gem.win_platform?
          result = described_class.with_timeout("cmd", "/c", "exit 42")
        else
          result = described_class.with_timeout("sh", "-c", "exit 42")
        end

        expect(result[:status].success?).to be false
        expect(result[:status].exitstatus).to eq(42)
        expect(result[:timeout]).to be false
      end
    end

    context "with stdin_data" do
      it "sends input to command" do
        if Gem.win_platform?
          # Windows: use PowerShell to read from stdin
          result = described_class.with_timeout(
            "powershell", "-Command", "$input",
            stdin_data: "test input"
          )
        else
          # Unix: use cat to read from stdin
          result = described_class.with_timeout(
            "cat",
            stdin_data: "test input"
          )
        end

        expect(result[:stdout].strip).to eq("test input")
        expect(result[:status].success?).to be true
      end
    end

    context "with timeout" do
      it "terminates long-running command" do
        if Gem.win_platform?
          # Windows: use ruby -e to create a sleep - most reliable cross-platform
          # This command will take about 10 seconds to complete
          result = described_class.with_timeout(
            "ruby", "-e", "sleep 10",
            timeout: 1
          )
        else
          # Unix: use sleep
          result = described_class.with_timeout(
            "sleep", "10",
            timeout: 1
          )
        end

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
          binmode: true
        )

        expect(result[:stdout]).to eq(binary_data)
        expect(result[:status].success?).to be true
      end
    end

    context "with environment variables" do
      it "passes environment to command" do
        if Gem.win_platform?
          result = described_class.with_timeout(
            { "TEST_VAR" => "test_value" },
            "powershell", "-Command", "echo $env:TEST_VAR"
          )
        else
          result = described_class.with_timeout(
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
            chdir: temp_dir
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
        if Gem.win_platform?
          # Windows only supports KILL signal
          # Use ruby -e to create a sleep - most reliable cross-platform
          result = described_class.with_timeout(
            "ruby", "-e", "sleep 10",
            timeout: 1,
            signal: :KILL
          )
        else
          # Unix can use TERM signal
          result = described_class.with_timeout(
            "sleep", "10",
            timeout: 1,
            signal: :TERM
          )
        end

        expect(result[:timeout]).to be true
      end
    end

    context "edge cases" do
      it "handles commands that exit immediately" do
        if Gem.win_platform?
          result = described_class.with_timeout("cmd", "/c", "exit 0")
        else
          result = described_class.with_timeout("true")
        end

        expect(result[:status].success?).to be true
        expect(result[:timeout]).to be false
      end

      it "handles empty output" do
        if Gem.win_platform?
          result = described_class.with_timeout("cmd", "/c", "")
        else
          result = described_class.with_timeout("true")
        end

        expect(result[:stdout]).to be_empty
        expect(result[:stderr]).to be_empty
        expect(result[:status].success?).to be true
      end

      it "handles large output" do
        # Generate large output (platform-specific)
        if Gem.win_platform?
          result = described_class.with_timeout(
            "powershell", "-Command",
            "1..1000 | ForEach-Object { Write-Output $_ }"
          )
        else
          result = described_class.with_timeout(
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
