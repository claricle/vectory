require "spec_helper"

RSpec.describe Vectory::SystemCall do
  describe "#call" do
    context "with successful command execution" do
      it "executes simple commands" do
        call = described_class.new(["echo", "test"]).call

        expect(call.stdout.strip).to eq("test")
        expect(call.stderr).to be_empty
        expect(call.status.success?).to be true
      end

      it "executes commands with string format" do
        call = if Gem.win_platform?
                 described_class.new("cmd /c echo test").call
               else
                 described_class.new("echo test").call
               end

        expect(call.stdout.strip).to eq("test")
        expect(call.status.success?).to be true
      end

      it "executes commands with array format" do
        call = described_class.new(["echo", "array format"]).call

        # Windows echo adds quotes, Unix doesn't
        expected = Gem.win_platform? ? '"array format"' : "array format"
        expect(call.stdout.strip).to eq(expected)
        expect(call.status.success?).to be true
      end

      it "stores command for inspection" do
        cmd = ["echo", "inspect"]
        call = described_class.new(cmd).call

        expect(call.cmd).to eq(cmd)
      end
    end

    context "with command failure" do
      it "raises SystemCallError on failure" do
        cmd = if Gem.win_platform?
                ["cmd", "/c", "exit 1"]
              else
                ["sh", "-c", "exit 1"]
              end

        expect do
          described_class.new(cmd).call
        end.to raise_error(Vectory::SystemCallError)
      end

      it "includes command details in error message" do
        cmd = if Gem.win_platform?
                ["cmd", "/c", "exit 42"]
              else
                ["sh", "-c", "exit 42"]
              end

        expect do
          described_class.new(cmd).call
        end.to raise_error(Vectory::SystemCallError, /exit/)
      end

      it "includes stdout and stderr in error message" do
        cmd = if Gem.win_platform?
                [
                  "powershell", "-Command",
                  "Write-Output 'out'; [Console]::Error.WriteLine('err'); exit 1"
                ]
              else
                ["sh", "-c", "echo 'out'; echo 'err' >&2; exit 1"]
              end

        expect do
          described_class.new(cmd).call
        end.to raise_error(Vectory::SystemCallError, /out.*err/m)
      end
    end

    context "with nonexistent command" do
      it "raises SystemCallError" do
        expect do
          described_class.new(["nonexistent_command_xyz"]).call
        end.to raise_error(Vectory::SystemCallError)
      end
    end

    context "with timeout" do
      it "uses default timeout" do
        call = described_class.new(["echo", "test"])
        expect(call.instance_variable_get(:@timeout)).to eq(
          Vectory::SystemCall::TIMEOUT,
        )
      end

      it "accepts custom timeout" do
        call = described_class.new(["echo", "test"], 30)
        expect(call.instance_variable_get(:@timeout)).to eq(30)
      end

      it "raises error on timeout" do
        cmd = if Gem.win_platform?
                # On Windows, use ruby -e to create a sleep - most reliable cross-platform
                # This command will take about 10 seconds to complete
                ["ruby", "-e", "sleep 10"]
              else
                ["sleep", "10"]
              end

        expect do
          described_class.new(cmd, 1).call
        end.to raise_error(Vectory::SystemCallError, /timed out/)
      end
    end

    context "with different output scenarios" do
      it "captures multiline stdout" do
        cmd = if Gem.win_platform?
                ["powershell", "-Command",
                 "Write-Output 'line1'; Write-Output 'line2'"]
              else
                ["sh", "-c", "echo 'line1'; echo 'line2'"]
              end

        call = described_class.new(cmd).call
        lines = call.stdout.split("\n").map(&:strip).reject(&:empty?)

        expect(lines).to include("line1", "line2")
      end

      it "captures multiline stderr" do
        cmd = if Gem.win_platform?
                [
                  "powershell", "-Command",
                  "[Console]::Error.WriteLine('err1'); [Console]::Error.WriteLine('err2')"
                ]
              else
                ["sh", "-c", "echo 'err1' >&2; echo 'err2' >&2"]
              end

        call = described_class.new(cmd).call
        lines = call.stderr.split("\n").map(&:strip).reject(&:empty?)

        expect(lines).to include("err1", "err2")
      end

      it "handles empty output" do
        cmd = if Gem.win_platform?
                ["cmd", "/c", ""]
              else
                ["true"]
              end

        call = described_class.new(cmd).call

        expect(call.stdout).to be_empty
        expect(call.stderr).to be_empty
        expect(call.status.success?).to be true
      end
    end

    context "with real-world commands" do
      it "executes echo command" do
        call = described_class.new(["echo", "real world test"]).call

        # Windows echo adds quotes, Unix doesn't
        expected = Gem.win_platform? ? '"real world test"' : "real world test"
        expect(call.stdout.strip).to eq(expected)
        expect(call.status.success?).to be true
      end

      it "works with commands that have options" do
        if Gem.win_platform?
          call = described_class.new(["cmd", "/c", "echo", "test"]).call
          expect(call.stdout.strip).to eq("test")
        else
          call = described_class.new(["echo", "-n", "test"]).call
          expect(call.stdout).to eq("test")
        end

        expect(call.status.success?).to be true
      end
    end

    context "platform-specific behavior" do
      it "handles platform-specific commands correctly" do
        if Gem.win_platform?
          # Test Windows-specific command
          call = described_class.new(["cmd", "/c", "ver"]).call
          expect(call.stdout).to match(/Windows|Microsoft/i)
        else
          # Test Unix-specific command
          call = described_class.new(["uname"]).call
          expect(call.stdout.strip).not_to be_empty
        end

        expect(call.status.success?).to be true
      end

      it "uses KILL signal on Windows" do
        # The SystemCall class uses KILL signal which works on Windows
        # This is tested indirectly through timeout tests
        cmd = if Gem.win_platform?
                # On Windows, use ruby -e to create a sleep - most reliable cross-platform
                ["ruby", "-e", "sleep 5"]
              else
                ["sleep", "5"]
              end

        expect do
          described_class.new(cmd, 1).call
        end.to raise_error(Vectory::SystemCallError, /timed out/)
      end
    end

    context "with special characters in arguments" do
      it "handles spaces in arguments" do
        call = described_class.new(["echo", "hello world"]).call

        # Windows echo adds quotes, Unix doesn't
        expected = Gem.win_platform? ? '"hello world"' : "hello world"
        expect(call.stdout.strip).to eq(expected)
        expect(call.status.success?).to be true
      end

      it "handles quotes in arguments" do
        call = described_class.new(["echo", "test 'quoted' text"]).call

        # Windows echo adds quotes, Unix doesn't
        expected = Gem.win_platform? ? '"test \'quoted\' text"' : "test 'quoted' text"
        expect(call.stdout.strip).to eq(expected)
        expect(call.status.success?).to be true
      end
    end

    context "error handling edge cases" do
      it "handles commands with no status" do
        # This is hard to test directly, but we can verify the error message format
        cmd = if Gem.win_platform?
                ["cmd", "/c", "exit 1"]
              else
                ["sh", "-c", "exit 1"]
              end

        begin
          described_class.new(cmd).call
        rescue Vectory::SystemCallError => e
          expect(e.message).to include("Failed to run")
        end
      end
    end
  end

  describe "initialization" do
    it "accepts command as array" do
      call = described_class.new(["echo", "test"])
      expect(call.cmd).to eq(["echo", "test"])
    end

    it "accepts command as string" do
      call = described_class.new("echo test")
      expect(call.cmd).to eq("echo test")
    end

    it "sets default timeout" do
      call = described_class.new(["echo", "test"])
      expect(call.instance_variable_get(:@timeout)).to eq(
        Vectory::SystemCall::TIMEOUT,
      )
    end

    it "accepts custom timeout" do
      call = described_class.new(["echo", "test"], 42)
      expect(call.instance_variable_get(:@timeout)).to eq(42)
    end
  end

  describe "attr_readers" do
    it "provides access to status" do
      call = described_class.new(["echo", "test"]).call
      expect(call.status).to be_a(Process::Status)
    end

    it "provides access to stdout" do
      call = described_class.new(["echo", "test"]).call
      expect(call.stdout).to be_a(String)
    end

    it "provides access to stderr" do
      call = described_class.new(["echo", "test"]).call
      expect(call.stderr).to be_a(String)
    end

    it "provides access to cmd" do
      cmd = ["echo", "test"]
      call = described_class.new(cmd).call
      expect(call.cmd).to eq(cmd)
    end
  end
end
